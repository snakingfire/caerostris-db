//! Unit tests for the `.ncol` columnar node-property writer/reader (T-0007).
//!
//! These exercise ADR 0008 §2: round-trip fidelity over every [`PropertyValue`]
//! kind, the self-describing framing (header/trailer/directory), fail-closed
//! version/magic/codec handling, and the columnar (single-column) read path —
//! the substrate for land-gate condition **C3** (asserted byte-exactly in the
//! integration test `tests/ncol_columnar_read.rs`).

use super::*;
use crate::model::{Node, NodeId, PropertyValue};
use crate::storage::MemoryStore;
use std::collections::BTreeMap;

const KEY: &str = "db/data/test/nodes-Person-0.ncol";

/// Write `nodes` to a fresh [`MemoryStore`] at [`KEY`] and return the store +
/// the shard's directory.
fn put_shard(nodes: &[Node]) -> (MemoryStore, ColumnDir) {
    let shard = NcolWriter.serialize(nodes).expect("serialize");
    let mut store = MemoryStore::new();
    store.put(KEY, shard.bytes.clone()).expect("put");
    (store, shard.dir)
}

fn person(id: u64) -> Node {
    Node::new(NodeId(id))
        .with_label("Person")
        .with_property("name", format!("n{id}"))
        .with_property("age", id as i64)
}

#[test]
fn header_constants_match_layout() {
    // Guards against a silent framing drift if a field is added/removed.
    assert_eq!(HEADER_LEN, 46);
    assert_eq!(TRAILER_LEN, 16);
}

#[test]
fn serialize_empty_shard_is_error() {
    let err = NcolWriter.serialize(&[]).unwrap_err();
    assert!(matches!(err, NcolError::Malformed(_)));
}

#[test]
fn round_trip_single_node_all_scalar_types() {
    let mut props = BTreeMap::new();
    props.insert("b".to_string(), PropertyValue::Boolean(true));
    props.insert("i".to_string(), PropertyValue::Integer(-42));
    props.insert("f".to_string(), PropertyValue::Float(3.5));
    props.insert("s".to_string(), PropertyValue::String("héllo".into()));
    props.insert("nul".to_string(), PropertyValue::Null);
    let node = Node {
        id: NodeId(7),
        labels: ["A".to_string(), "B".to_string()].into_iter().collect(),
        properties: props,
    };
    let (store, dir) = put_shard(std::slice::from_ref(&node));
    let back = NcolReader::read_all(&store, KEY, &dir).expect("read_all");
    assert_eq!(back, vec![node]);
}

#[test]
fn round_trip_containers_list_and_map() {
    let mut inner = BTreeMap::new();
    inner.insert("k".to_string(), PropertyValue::Integer(1));
    inner.insert("z".to_string(), PropertyValue::Null);
    let list = PropertyValue::List(vec![
        PropertyValue::Integer(1),
        PropertyValue::String("x".into()),
        PropertyValue::Map(inner.clone()),
    ]);
    let node = Node::new(NodeId(3))
        .with_label("Thing")
        .with_property("list", list)
        .with_property("map", PropertyValue::Map(inner));
    let (store, dir) = put_shard(std::slice::from_ref(&node));
    let back = NcolReader::read_all(&store, KEY, &dir).expect("read_all");
    assert_eq!(back, vec![node]);
}

#[test]
fn round_trip_preserves_missing_vs_null_property() {
    // n10 has "opt" set to Null (present); n11 never sets "opt" (absent).
    let n10 = Node::new(NodeId(10)).with_property("opt", PropertyValue::Null);
    let n11 = Node::new(NodeId(11)).with_property("other", 1_i64);
    let (store, dir) = put_shard(&[n10.clone(), n11.clone()]);
    let back = NcolReader::read_all(&store, KEY, &dir).expect("read_all");
    assert_eq!(back.len(), 2);
    // n10: "opt" present-and-null → Some(Null); n11: "opt" absent → None.
    assert_eq!(back[0].property("opt"), Some(&PropertyValue::Null));
    assert_eq!(back[1].property("opt"), None);
    assert_eq!(back[0], n10);
    assert_eq!(back[1], n11);
}

#[test]
fn nodes_are_sorted_by_id_regardless_of_input_order() {
    // Sparse ids (1,2,5,9) within the band [1,9]: the reserved :id column lets
    // the reader reconstruct exactly the stored nodes, in id order — no phantom
    // rows for the gaps.
    let nodes = vec![person(5), person(2), person(9), person(1)];
    let (store, dir) = put_shard(&nodes);
    assert_eq!(dir.id_band, (1, 9));
    let back = NcolReader::read_all(&store, KEY, &dir).expect("read_all");
    let ids: Vec<u64> = back.iter().map(|n| n.id.0).collect();
    assert_eq!(ids, vec![1, 2, 5, 9]);
    assert_eq!(back[3].property("age"), Some(&PropertyValue::Integer(9)));
}

#[test]
fn duplicate_id_in_shard_is_error() {
    let err = NcolWriter.serialize(&[person(1), person(1)]).unwrap_err();
    assert!(matches!(err, NcolError::Malformed(_)));
}

#[test]
fn read_dir_rediscovers_directory_from_object_bytes() {
    // The self-describing path: a reader that knows only the key can recover
    // the directory via the trailer/header without the manifest.
    let (store, written_dir) = put_shard(&[person(1), person(2)]);
    let discovered = NcolReader::read_dir(&store, KEY).expect("read_dir");
    assert_eq!(discovered, written_dir);
}

#[test]
fn read_column_returns_per_row_values() {
    let nodes = vec![person(1), person(2), person(3)];
    let (store, dir) = put_shard(&nodes);
    let ages = NcolReader::read_column(&store, KEY, &dir, "age").expect("read_column");
    assert_eq!(
        ages,
        vec![
            Some(PropertyValue::Integer(1)),
            Some(PropertyValue::Integer(2)),
            Some(PropertyValue::Integer(3)),
        ]
    );
}

#[test]
fn read_column_unknown_property_is_error() {
    let (store, dir) = put_shard(&[person(1)]);
    let err = NcolReader::read_column(&store, KEY, &dir, "no_such").unwrap_err();
    assert!(matches!(err, NcolError::NoSuchColumn(_)));
}

#[test]
fn read_nodes_in_id_range_returns_only_in_range_rows() {
    // Dense band [100..104]; ask for [101, 103].
    let nodes: Vec<Node> = (100..=104).map(person).collect();
    let (store, dir) = put_shard(&nodes);
    assert_eq!(dir.id_band, (100, 104));
    let got = NcolReader::read_nodes_in_id_range(&store, KEY, &dir, 101, 103).expect("range");
    let ids: Vec<u64> = got.iter().map(|n| n.id.0).collect();
    assert_eq!(ids, vec![101, 102, 103]);
}

#[test]
fn columnar_read_fetches_fewer_bytes_than_whole_object() {
    // C3 in miniature (the integration test asserts the budget share): reading
    // one column's chunk must transfer strictly fewer bytes than the object.
    let nodes: Vec<Node> = (0..64).map(person).collect();
    let shard = NcolWriter.serialize(&nodes).expect("serialize");
    let object_len = shard.bytes.len();
    let age = shard.dir.column("age").expect("age column");
    let column_span = (age.chunk_off + age.chunk_len - age.present_bitmap_off) as usize;
    assert!(
        column_span < object_len,
        "column span {column_span} should be < object {object_len}"
    );
}

// ---- fail-closed framing -------------------------------------------------

#[test]
fn reader_rejects_bad_magic() {
    let (store, _) = put_shard(&[person(1)]);
    // Corrupt the first byte of the magic in a fresh store.
    let mut bytes = store.get(KEY).unwrap();
    bytes[0] ^= 0xFF;
    let mut s = MemoryStore::new();
    s.put(KEY, bytes).unwrap();
    let err = NcolReader::read_dir(&s, KEY).unwrap_err();
    assert!(matches!(err, NcolError::BadMagic(_)));
}

#[test]
fn reader_rejects_unsupported_version_fail_closed() {
    let (store, _) = put_shard(&[person(1)]);
    let mut bytes = store.get(KEY).unwrap();
    // format_version is the u16 at offset 4.
    bytes[4] = 0xFE;
    bytes[5] = 0xFF;
    let mut s = MemoryStore::new();
    s.put(KEY, bytes).unwrap();
    let err = NcolReader::read_dir(&s, KEY).unwrap_err();
    assert!(matches!(err, NcolError::UnsupportedVersion(_)));
}

#[test]
fn reader_rejects_wrong_object_kind() {
    let (store, _) = put_shard(&[person(1)]);
    let mut bytes = store.get(KEY).unwrap();
    bytes[6] = 99; // object_kind
    let mut s = MemoryStore::new();
    s.put(KEY, bytes).unwrap();
    let err = NcolReader::read_dir(&s, KEY).unwrap_err();
    assert!(matches!(err, NcolError::WrongObjectKind(_)));
}

#[test]
fn truncated_header_is_error() {
    let mut s = MemoryStore::new();
    s.put(KEY, vec![0u8; 4]).unwrap();
    let err = NcolReader::read_dir(&s, KEY).unwrap_err();
    assert!(matches!(
        err,
        NcolError::Truncated { .. } | NcolError::BadMagic(_)
    ));
}

#[test]
fn missing_object_propagates_store_error() {
    let s = MemoryStore::new();
    let err = NcolReader::read_dir(&s, "absent").unwrap_err();
    assert!(matches!(err, NcolError::Store(_)));
}

#[test]
fn labels_round_trip_including_empty_label_set() {
    let n = Node::new(NodeId(1)); // no labels, no props
    let (store, dir) = put_shard(std::slice::from_ref(&n));
    let back = NcolReader::read_all(&store, KEY, &dir).expect("read_all");
    assert_eq!(back, vec![n]);
    assert!(back[0].labels.is_empty());
}

// ---- generative round-trip fidelity (AC3) --------------------------------
//
// AC3 calls for property-based round-trip fidelity over "arbitrary node sets
// ... (labels + all property types)". The engine deliberately avoids the `rand`
// crate (and therefore `proptest`, which pulls in ~19 transitive crates) to
// keep the lockfile lean and the license manifest small — the same call
// `src/dataset/rng.rs` already made. We instead drive a deterministic,
// reproducible generator (the repo's own SplitMix64) over many seeds, which
// gives the same coverage of the value space with zero new dependencies. A
// follow-up task may add a proptest harness if the dependency tree is justified
// at the workspace level. The seed is printed on failure for reproduction.

use crate::dataset::SplitMix64;

/// Build an arbitrary [`PropertyValue`] of bounded `depth` (containers nest
/// only while `depth > 0`), covering every variant including `Null`.
fn arb_value(g: &mut SplitMix64, depth: u32) -> PropertyValue {
    let kinds = if depth == 0 { 5 } else { 7 };
    match g.below(kinds) {
        0 => PropertyValue::Null,
        1 => PropertyValue::Boolean(g.below(2) == 1),
        2 => PropertyValue::Integer(g.next_u64() as i64),
        3 => {
            // Mix in some special floats (incl. a NaN) for total-order coverage.
            let pick = g.below(4);
            let f = match pick {
                0 => f64::NAN,
                1 => 0.0,
                2 => -1.5,
                _ => g.unit_f64() * 1e6 - 5e5,
            };
            PropertyValue::Float(f)
        }
        4 => {
            let len = g.below(6) as usize;
            let s: String = (0..len)
                .map(|_| char::from(b'a' + (g.below(26) as u8)))
                .collect();
            PropertyValue::String(s)
        }
        5 => {
            let n = g.below(4) as usize;
            PropertyValue::List((0..n).map(|_| arb_value(g, depth - 1)).collect())
        }
        _ => {
            let n = g.below(4) as usize;
            let mut m = BTreeMap::new();
            for k in 0..n {
                m.insert(format!("k{k}"), arb_value(g, depth - 1));
            }
            PropertyValue::Map(m)
        }
    }
}

/// Build an arbitrary node set with distinct ids in a (possibly sparse) band.
fn arb_nodes(g: &mut SplitMix64) -> Vec<Node> {
    let count = 1 + g.below(12) as usize;
    let mut ids = std::collections::BTreeSet::new();
    while ids.len() < count {
        ids.insert(g.below(1000));
    }
    let label_pool = ["Person", "Place", "Thing", "Event"];
    let key_pool = ["name", "age", "score", "tags", "meta"];
    ids.into_iter()
        .map(|id| {
            let mut node = Node::new(NodeId(id));
            let nlabels = g.below(label_pool.len() as u64 + 1) as usize;
            for _ in 0..nlabels {
                node.labels
                    .insert(label_pool[g.below(label_pool.len() as u64) as usize].to_string());
            }
            let nprops = g.below(key_pool.len() as u64 + 1) as usize;
            for _ in 0..nprops {
                let key = key_pool[g.below(key_pool.len() as u64) as usize];
                node.properties.insert(key.to_string(), arb_value(g, 2));
            }
            node
        })
        .collect()
}

/// Round-tripping any arbitrary node set returns the identical set; and the
/// serialised bytes are deterministic (re-serialising yields identical bytes),
/// which the content-addressed commit layer (T-0009/ADR 0002) relies on.
#[test]
fn generative_round_trip_fidelity_and_determinism() {
    for seed in 0..400u64 {
        let mut g = SplitMix64::new(seed);
        let nodes = arb_nodes(&mut g);

        let shard1 = NcolWriter.serialize(&nodes).expect("serialize");
        let shard2 = NcolWriter.serialize(&nodes).expect("serialize");
        assert_eq!(
            shard1.bytes, shard2.bytes,
            "seed {seed}: serialisation must be deterministic"
        );

        let mut store = MemoryStore::new();
        store.put(KEY, shard1.bytes).expect("put");
        let back = NcolReader::read_all(&store, KEY, &shard1.dir).expect("read_all");

        // Expected = the same nodes sorted by id (the shard ordering).
        let mut expected = nodes.clone();
        expected.sort_by_key(|n| n.id.0);
        assert_eq!(back, expected, "seed {seed}: round-trip mismatch");

        // The self-describing directory rediscovers identically.
        let rediscovered = NcolReader::read_dir(&store, KEY).expect("read_dir");
        assert_eq!(rediscovered, shard1.dir, "seed {seed}: dir mismatch");
    }
}
