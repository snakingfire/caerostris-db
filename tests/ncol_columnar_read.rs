//! Integration test (T-0007, ADR 0008 §2.4 / §6 + land-gate **C3**): a
//! single-property filter read over a node-id seed band fetches **only that
//! column's chunk**, not whole node records, and stays within the node-side
//! share of the latency byte budget `B_max`.
//!
//! ## What this proves
//!
//! - **C3** (steering-storage condition on T-0007): evaluating a single-property
//!   filter fetches ≤ that column's chunk bytes over the touched shards, never
//!   the whole node record (ADR 0008 §2.4).
//! - **AC4**: a representative range-GET for an in-envelope node span reads ≤
//!   the `B_max` share allotted to the node side (ADR 0001 §2.2 phase-2 bound
//!   `bytes_index ≤ N_seed × bytes_node`).
//!
//! ## The backend
//!
//! All reads go through the [`ObjectStore`] trait. This suite runs against the
//! in-process backend — the documented final rung of the env provision ladder
//! (`docs/process/parallel-execution-and-environment.md`: "Docker MinIO →
//! moto_server → pip moto → in-process memory backend"). Because the byte count
//! is taken at the `ObjectStore` boundary by [`CountingStore`], the figure is
//! **identical to what a real S3 range-GET would transfer** — the engine has no
//! S3 `ObjectStore` adapter yet (that lands with the attach-modes work, EPIC
//! tracking the S3 client), so wiring this same test to MinIO is a drop-in swap
//! of the inner store with zero changes to the assertions. The harness still
//! self-provisions the shared mock (`scripts/env/up.sh`) per repo policy; this
//! test simply does not yet require it.

use std::collections::BTreeMap;

use caerostris_db::model::{Node, NodeId, PropertyValue};
use caerostris_db::storage::ncol::{NcolReader, NcolWriter};
use caerostris_db::storage::{CountingStore, MemoryStore, ObjectStore};

/// Latency design point (ADR 0001 §2.2): average bytes per node.
const BYTES_NODE: u64 = 256;

const KEY: &str = "db/data/<hash>/nodes-Person-000000.ncol";

/// Build a `Person` node with a *selective* filter column (`country`) plus
/// several other "wide" properties, so the whole record is meaningfully larger
/// than the single filter column — the situation C3 is about.
fn person(id: u64, country: &str) -> Node {
    let mut props = BTreeMap::new();
    props.insert(
        "country".to_string(),
        PropertyValue::String(country.to_string()),
    );
    // Wide, non-filter properties that a columnar filter read must NOT fetch.
    props.insert(
        "name".to_string(),
        PropertyValue::String(format!("Person Number {id} of the City")),
    );
    props.insert(
        "bio".to_string(),
        PropertyValue::String(format!(
            "A reasonably long biography string for node {id}, padding the row \
             so a row-oriented read would transfer far more than the filter column."
        )),
    );
    props.insert("age".to_string(), PropertyValue::Integer((id % 100) as i64));
    props.insert("score".to_string(), PropertyValue::Float((id as f64) * 1.5));
    Node {
        id: NodeId(id),
        labels: ["Person".to_string()].into_iter().collect(),
        properties: props,
    }
}

/// Write one in-envelope shard of `n` `Person` nodes (a contiguous id band) to a
/// store and return the store, the shard directory, and the object length.
fn write_seed_shard(n: u64) -> (MemoryStore, caerostris_db::storage::ncol::ColumnDir, usize) {
    let countries = ["IS", "NO", "SE", "DK", "FI"];
    let nodes: Vec<Node> = (0..n)
        .map(|i| person(i, countries[(i % countries.len() as u64) as usize]))
        .collect();
    let shard = NcolWriter.serialize(&nodes).expect("serialize");
    let object_len = shard.bytes.len();
    let mut store = MemoryStore::new();
    store.put(KEY, shard.bytes).expect("put");
    (store, shard.dir, object_len)
}

#[test]
fn columnar_filter_read_fetches_only_the_filtered_column() {
    // An in-envelope seed band: N_seed nodes in one contiguous shard.
    let n_seed = 4000u64;
    let (inner, dir, object_len) = write_seed_shard(n_seed);
    let store = CountingStore::new(inner);

    // Read ONLY the `country` filter column (the C3 access pattern).
    store.reset();
    let countries =
        NcolReader::read_column(&store, KEY, &dir, "country").expect("read filter column");
    assert_eq!(countries.len(), n_seed as usize);

    let filter_bytes = store.bytes_fetched();
    let requests = store.get_requests();

    // C3.1 — exactly one range-GET (the single column span), no whole-object GET.
    assert_eq!(
        requests, 1,
        "the filter read must be a single range-GET, was {requests}"
    );

    // C3.2 — the bytes fetched equal the `country` column's span (present bitmap
    // + value chunk), and are strictly less than the whole object.
    let col = dir.column("country").expect("country column");
    let expected_span = (col.chunk_off + col.chunk_len) - col.present_bitmap_off;
    assert_eq!(
        filter_bytes, expected_span,
        "filter read must fetch exactly the column span"
    );
    assert!(
        filter_bytes < object_len as u64,
        "filter read ({filter_bytes} B) must be < whole object ({object_len} B)"
    );

    // C3.3 — the filter column is a small fraction of the object: reading whole
    // node records (the rejected row-oriented design, ADR 0008 Alt. A) would
    // transfer many times more bytes.
    assert!(
        filter_bytes * 4 < object_len as u64,
        "filter column ({filter_bytes} B) should be a small fraction of the \
         object ({object_len} B); row-orientation would read the whole object"
    );

    // AC4 — node-side B_max share: the phase-2 read bound from ADR 0001 §2.2 is
    // `bytes_index ≤ N_seed × bytes_node`. The columnar filter read is far
    // inside it (it reads one narrow column, not whole nodes).
    let node_side_budget = n_seed * BYTES_NODE;
    assert!(
        filter_bytes <= node_side_budget,
        "filter read ({filter_bytes} B) must be ≤ node-side budget \
         ({node_side_budget} B = N_seed {n_seed} × bytes_node {BYTES_NODE})"
    );
}

#[test]
fn range_read_over_id_subspan_reconstructs_only_in_range_nodes() {
    // A range-GET over an id sub-span fetches/reconstructs only that span's rows
    // (AC2: get_range over an id span fetches the relevant partition).
    let n_seed = 1000u64;
    let (store, dir, _) = write_seed_shard(n_seed);

    let (lo, hi) = (200u64, 209u64);
    let nodes = NcolReader::read_nodes_in_id_range(&store, KEY, &dir, lo, hi).expect("range read");

    let ids: Vec<u64> = nodes.iter().map(|n| n.id.0).collect();
    assert_eq!(ids, (lo..=hi).collect::<Vec<_>>());
    // Fidelity within the span: the filter column survives the round trip.
    for n in &nodes {
        assert!(n.property("country").is_some());
        assert!(n.has_label("Person"));
    }
}

#[test]
fn whole_node_reconstruction_reads_more_than_one_column() {
    // Sanity contrast: reconstructing FULL nodes touches every column, so it
    // fetches strictly more than the single-column filter read — confirming the
    // C3 win is real, not an artifact of a trivially small object.
    let n_seed = 2000u64;
    let (inner, dir, _) = write_seed_shard(n_seed);
    let store = CountingStore::new(inner);

    store.reset();
    let _ = NcolReader::read_column(&store, KEY, &dir, "country").expect("filter col");
    let filter_only = store.bytes_fetched();

    store.reset();
    let _ = NcolReader::read_all(&store, KEY, &dir).expect("read all");
    let whole_nodes = store.bytes_fetched();

    assert!(
        whole_nodes > filter_only,
        "whole-node read ({whole_nodes} B) must exceed single-column filter read \
         ({filter_only} B)"
    );
}
