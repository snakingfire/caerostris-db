//! Integration tests (rubric Cat. 2 / Cat. 3) for the CSR adjacency-list `.adj`
//! writer + chunked range reader (board item T-0008, ADR 0008 §3).
//!
//! These exercise the writer and reader **through the
//! [`ObjectStore`](caerostris_db::storage::ObjectStore) trait** — the same
//! interface the local S3 mock (MinIO/moto) is wired behind — using the
//! in-process [`MemoryStore`] backend. The storage layer talks only to
//! `ObjectStore`, so a passing test here is a passing test against any backend
//! that honours the trait's `get` / `get_range` / `put` contract; the env
//! scripts (`scripts/env/up.sh`, `scripts/env/bucket.sh`) provision the MinIO
//! endpoint, and `MemoryStore` is the documented no-Docker fallback backend
//! (`scripts/env/up.sh` ladder step (d)). When the S3 client adapter lands
//! (its own task), these same assertions run unchanged against MinIO.
//!
//! The headline assertion is **AC #4**: a single-hop expansion for an
//! in-envelope frontier reads no more than its allotted `B_max` share, and the
//! §3.4 hard per-GET byte cap keeps a super-hub frontier node within budget.

use std::collections::BTreeMap;

use caerostris_db::model::{Edge, PropertyValue};
use caerostris_db::storage::MemoryStore;
use caerostris_db::storage::{
    AdjacencyShardReader, AdjacencyShardWriter, Direction, ExpandCap, ObjectStore,
    StorageFormatError,
};

// ---------------------------------------------------------------------------
// ADR 0001 / ADR 0008 §6 design points used to size the in-envelope budget.
// ---------------------------------------------------------------------------

/// Binding 50 Mbps byte budget for the whole 6-hop query (ADR 0001 §1.7).
const B_MAX_50MBPS: usize = 2_880_000;
/// Frontier-width cap: max parallel GETs per hop (ADR 0001 §1.7 design point).
const M_MAX: usize = 8;
/// Per-hop tail out-degree design point (ADR 0001 §6.2 worked example).
const F_TAIL: usize = 10;
/// Average bytes per edge row design point (ADR 0001 Part 5).
const BYTES_EDGE_ROW: usize = 64;

/// The per-hop byte share a single hop is allotted in the §6.1 phase
/// accounting: one parallel batch of `M_MAX` neighbour-block reads, each a
/// frontier node of `F_TAIL` edges. `8 * 10 * 64 = 5120` bytes — a tiny
/// fraction of the 2.88 MB whole-query budget.
const PER_HOP_SHARE: usize = M_MAX * F_TAIL * BYTES_EDGE_ROW;

const KEY: &str = "db/data/abc123/adj-FOLLOWS-out-000000.adj";

/// Build one out-shard from `edges` over band `[lo, hi]` and store it.
fn write_shard(store: &mut MemoryStore, edges: &[Edge], lo: u64, hi: u64) {
    let mut w = AdjacencyShardWriter::new(1, Direction::Out, lo, hi);
    for e in edges {
        w.push(e);
    }
    store.put(KEY, w.finish()).unwrap();
}

#[test]
fn write_read_round_trip_through_object_store() {
    // A small realistic graph: 64 source nodes, fan-out ~F_TAIL each.
    let mut edges = Vec::new();
    let mut eid = 0u64;
    for src in 0..64u64 {
        for j in 0..(F_TAIL as u64) {
            let tgt = 1000 + src * 100 + j;
            let e = Edge::new(eid, "FOLLOWS", src, tgt)
                .with_property("weight", (j as i64) - 5)
                .with_property("label", format!("e{eid}"));
            edges.push(e);
            eid += 1;
        }
    }

    let mut store = MemoryStore::new();
    write_shard(&mut store, &edges, 0, 63);

    let reader = AdjacencyShardReader::open(store, KEY).unwrap();
    assert_eq!(reader.src_band(), (0, 63));

    // Every source reads back its exact neighbour set, including properties.
    for src in 0..64u64 {
        let nbrs = reader.neighbors(src).unwrap();
        assert_eq!(nbrs.len(), F_TAIL, "source {src} fan-out");
        for (j, n) in nbrs.iter().enumerate() {
            assert_eq!(n.neighbor.get(), 1000 + src * 100 + j as u64);
            assert_eq!(
                n.properties.get("weight"),
                Some(&PropertyValue::Integer(j as i64 - 5))
            );
            assert!(matches!(
                n.properties.get("label"),
                Some(PropertyValue::String(_))
            ));
        }
    }
}

#[test]
fn in_envelope_single_hop_reads_within_b_max_share() {
    // AC #4: an in-envelope frontier of M_MAX source nodes, each F_TAIL
    // out-edges, expanded for one hop must read <= its allotted B_max share.
    let mut edges = Vec::new();
    let mut eid = 0u64;
    for src in 0..(M_MAX as u64) {
        for j in 0..(F_TAIL as u64) {
            edges.push(Edge::new(eid, "FOLLOWS", src, 5000 + src * 50 + j));
            eid += 1;
        }
    }
    let mut store = MemoryStore::new();
    write_shard(&mut store, &edges, 0, (M_MAX - 1) as u64);
    let reader = AdjacencyShardReader::open(store, KEY).unwrap();

    // Expand the whole M_MAX-wide frontier, one bounded read per source.
    let mut total_bytes = 0usize;
    let mut total_gets = 0usize;
    let mut total_neighbors = 0usize;
    for src in 0..(M_MAX as u64) {
        // The reader is handed a budget for this hop; pass the per-hop share so
        // the cap is exercised end-to-end.
        let exp = reader.expand(src, ExpandCap::bytes(PER_HOP_SHARE)).unwrap();
        total_bytes += exp.bytes_read;
        total_gets += exp.gets;
        total_neighbors += exp.neighbors.len();
        assert!(
            !exp.truncated,
            "an in-envelope node must not need truncation"
        );
    }

    assert_eq!(total_neighbors, M_MAX * F_TAIL, "frontier fully expanded");
    // r <= 1: each source costs at most 2 range-GETs (directory + block).
    assert!(
        total_gets <= 2 * M_MAX,
        "hop must be a bounded batch of <= 2*M_MAX GETs, got {total_gets}"
    );
    // The headline AC: the whole hop's realized bytes are a tiny fraction of the
    // 2.88 MB 50 Mbps whole-query budget, and within the per-hop share * M_MAX.
    assert!(
        total_bytes <= PER_HOP_SHARE * M_MAX,
        "hop read {total_bytes} B exceeded its M_MAX * per-hop share"
    );
    assert!(
        total_bytes < B_MAX_50MBPS,
        "hop read {total_bytes} B exceeded the whole-query B_max ({B_MAX_50MBPS} B)"
    );
}

#[test]
fn super_hub_frontier_is_hard_capped_below_budget() {
    // ADR 0008 §3.4 / C2: a super-hub source with a degree far above p99 must
    // NOT cause a read beyond the cap — the directory exposes block_len before
    // the bytes, so the read is truncated.
    let mut edges = Vec::new();
    for j in 0..40_000u64 {
        edges.push(Edge::new(j, "FOLLOWS", 0, 1_000_000 + j));
    }
    let mut store = MemoryStore::new();
    write_shard(&mut store, &edges, 0, 0);
    let reader = AdjacencyShardReader::open(store, KEY).unwrap();

    // The full block is large; the directory degree alone proves it.
    assert_eq!(reader.degree(0).unwrap(), 40_000);

    // Expand under the per-hop share: realized bytes stay within the cap.
    let exp = reader.expand(0, ExpandCap::bytes(PER_HOP_SHARE)).unwrap();
    assert!(
        exp.truncated,
        "a super-hub read must be truncated by the cap"
    );
    assert!(
        exp.bytes_read <= PER_HOP_SHARE + 16,
        "super-hub read {} B busted the per-GET cap {PER_HOP_SHARE} B",
        exp.bytes_read
    );
    // It still produced a valid, in-order prefix of real neighbours.
    assert!(!exp.neighbors.is_empty());
    for w in exp.neighbors.windows(2) {
        assert!(w[0].neighbor.get() < w[1].neighbor.get());
    }
}

#[test]
fn directory_probe_is_one_small_range_get() {
    // The O(1) degree probe must read exactly one 16-byte directory entry —
    // never the whole object (the basis for OOE super-hub rejection without a
    // data-plane round-trip beyond the directory slice).
    let edges: Vec<Edge> = (0..100u64)
        .map(|j| Edge::new(j, "FOLLOWS", 3, 2000 + j))
        .collect();
    let mut store = MemoryStore::new();
    write_shard(&mut store, &edges, 0, 7);
    let reader = AdjacencyShardReader::open(store, KEY).unwrap();

    // expand with max_neighbors == 0 issues only the directory probe.
    let exp = reader
        .expand(
            3,
            ExpandCap {
                max_bytes: usize::MAX,
                max_neighbors: 0,
            },
        )
        .unwrap();
    assert_eq!(exp.gets, 1, "degree must be readable in one range-GET");
    assert_eq!(exp.bytes_read, 16, "directory entry is a fixed 16 bytes");
    assert!(exp.neighbors.is_empty());

    assert_eq!(reader.degree(3).unwrap(), 100);
}

#[test]
fn bidirectional_shards_serve_in_and_out_traversal() {
    // ADR 0008 §3.1: both directions materialised so in-edge traversal is also
    // r <= 1. Build an out-shard and an in-shard over the same edges and check
    // they agree on the graph.
    let edges = vec![
        Edge::new(1, "FOLLOWS", 10, 99),
        Edge::new(2, "FOLLOWS", 11, 99),
        Edge::new(3, "FOLLOWS", 10, 88),
    ];

    let mut out_store = MemoryStore::new();
    let mut ow = AdjacencyShardWriter::new(1, Direction::Out, 10, 11);
    for e in &edges {
        ow.push(e);
    }
    out_store.put("out.adj", ow.finish()).unwrap();
    let out = AdjacencyShardReader::open(out_store, "out.adj").unwrap();

    let mut in_store = MemoryStore::new();
    let mut iw = AdjacencyShardWriter::new(1, Direction::In, 88, 99);
    for e in &edges {
        iw.push(e);
    }
    in_store.put("in.adj", iw.finish()).unwrap();
    let inb = AdjacencyShardReader::open(in_store, "in.adj").unwrap();

    // Out: node 10 -> {88, 99}; node 11 -> {99}.
    let out10: Vec<u64> = out
        .neighbors(10)
        .unwrap()
        .iter()
        .map(|n| n.neighbor.get())
        .collect();
    assert_eq!(out10, vec![88, 99]);
    let out11: Vec<u64> = out
        .neighbors(11)
        .unwrap()
        .iter()
        .map(|n| n.neighbor.get())
        .collect();
    assert_eq!(out11, vec![99]);

    // In: node 99 <- {10, 11}; node 88 <- {10}.
    let in99: Vec<u64> = inb
        .neighbors(99)
        .unwrap()
        .iter()
        .map(|n| n.neighbor.get())
        .collect();
    assert_eq!(in99, vec![10, 11]);
    let in88: Vec<u64> = inb
        .neighbors(88)
        .unwrap()
        .iter()
        .map(|n| n.neighbor.get())
        .collect();
    assert_eq!(in88, vec![10]);
}

#[test]
fn corrupt_object_in_store_fails_closed() {
    // A flipped byte anywhere in a stored object must fail the open (fail-closed
    // integrity; ADR 0008 §8.2). Proves the reader never silently mis-reads
    // corrupted object-store bytes.
    let edges = vec![Edge::new(1, "FOLLOWS", 0, 7)];
    let mut store = MemoryStore::new();
    write_shard(&mut store, &edges, 0, 0);

    let mut bytes = store.get(KEY).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0x80;
    store.put(KEY, bytes).unwrap();

    let err = AdjacencyShardReader::open(store, KEY).unwrap_err();
    assert!(
        matches!(
            err,
            StorageFormatError::ChecksumMismatch | StorageFormatError::Truncated { .. }
        ),
        "corruption must fail closed, got {err:?}"
    );
}

#[test]
fn empty_edge_properties_round_trip() {
    // An edge with no properties is the common case; ensure it round-trips and
    // the property map comes back empty (not a sentinel).
    let edges = vec![Edge::new(42, "FOLLOWS", 0, 1)];
    let mut store = MemoryStore::new();
    write_shard(&mut store, &edges, 0, 0);
    let reader = AdjacencyShardReader::open(store, KEY).unwrap();
    let nbrs = reader.neighbors(0).unwrap();
    assert_eq!(nbrs.len(), 1);
    assert_eq!(nbrs[0].properties, BTreeMap::new());
}
