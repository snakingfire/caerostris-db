//! Unit tests for [`HashIndex`] — the second, equality-only index type.
//!
//! These exercise the **trait conformance** of a concrete index type whose key
//! shape has **no total order** (a hash multimap), proving the
//! [`SecondaryIndex`](crate::index::SecondaryIndex) trait carries no
//! B-tree-specific assumptions (T-0025 / Cat. 5 = 100 extensibility anchor).
//! The same suite also drives the index through the type-erased
//! [`PropertyIndex`](crate::index::PropertyIndex) planner facade — the proof that
//! the planner consults this index through the **same** trait-based API as the
//! B-tree, with zero knowledge of the concrete type.

use crate::index::{
    HashIndex, IndexCapabilities, IndexError, IndexQuery, KeyRange, OrderedKey, PropertyIndex,
    SecondaryIndex,
};
use crate::model::{NodeId, PropertyValue};

// --- helpers ----------------------------------------------------------------

/// A string-valued index key.
fn pv(s: &str) -> OrderedKey {
    OrderedKey(PropertyValue::String(s.to_string()))
}

/// A string [`PropertyValue`], as carried in a planner-facing [`IndexQuery`].
fn qv(s: &str) -> PropertyValue {
    PropertyValue::String(s.to_string())
}

fn node(id: u64) -> NodeId {
    NodeId(id)
}

/// A property→node [`HashIndex`] built from `(value, node)` pairs.
fn hash_index(pairs: &[(&str, u64)]) -> HashIndex<OrderedKey, NodeId> {
    let mut idx = HashIndex::new();
    for (v, n) in pairs {
        idx.insert(pv(v), node(*n));
    }
    idx
}

// --- construction & capabilities --------------------------------------------

#[test]
fn new_hash_index_is_empty() {
    let idx: HashIndex<OrderedKey, NodeId> = HashIndex::new();
    assert!(idx.is_empty());
    assert_eq!(idx.entry_count(), 0);
    assert_eq!(idx.lookup(&pv("anything")), Vec::<NodeId>::new());
}

#[test]
fn default_constructs_an_empty_index() {
    let idx: HashIndex<OrderedKey, NodeId> = HashIndex::default();
    assert!(idx.is_empty());
}

#[test]
fn hash_index_advertises_equality_only_capabilities() {
    let idx: HashIndex<OrderedKey, NodeId> = HashIndex::new();
    assert_eq!(idx.capabilities(), IndexCapabilities::equality_only());
    assert!(!idx.capabilities().supports_range);
    assert!(!idx.capabilities().supports_prefix);
}

// --- insert / lookup / delete (the equality-only contract) ------------------

#[test]
fn insert_then_lookup_returns_value() {
    let idx = hash_index(&[("alice", 1)]);
    assert_eq!(idx.lookup(&pv("alice")), vec![node(1)]);
    assert_eq!(idx.entry_count(), 1);
    assert!(!idx.is_empty());
}

#[test]
fn key_maps_to_many_values_in_insertion_order() {
    // Many nodes can share a property value — a non-unique secondary index.
    let idx = hash_index(&[("smith", 1), ("smith", 2), ("smith", 3)]);
    assert_eq!(idx.lookup(&pv("smith")), vec![node(1), node(2), node(3)]);
    assert_eq!(idx.entry_count(), 3);
}

#[test]
fn insert_is_idempotent_per_entry() {
    let mut idx: HashIndex<OrderedKey, NodeId> = HashIndex::new();
    assert!(idx.insert(pv("x"), node(1)), "first insert is new");
    assert!(
        !idx.insert(pv("x"), node(1)),
        "duplicate (key,value) is a no-op"
    );
    assert_eq!(idx.lookup(&pv("x")), vec![node(1)]);
    assert_eq!(idx.entry_count(), 1);
}

#[test]
fn delete_removes_only_the_named_entry() {
    let mut idx = hash_index(&[("smith", 1), ("smith", 2)]);
    assert!(idx.delete(&pv("smith"), &node(1)));
    assert_eq!(idx.lookup(&pv("smith")), vec![node(2)]);
    assert_eq!(idx.entry_count(), 1);
}

#[test]
fn delete_missing_entry_returns_false() {
    let mut idx = hash_index(&[("smith", 1)]);
    assert!(!idx.delete(&pv("smith"), &node(99)), "value not present");
    assert!(!idx.delete(&pv("jones"), &node(1)), "key not present");
    assert_eq!(idx.entry_count(), 1);
}

#[test]
fn deleting_last_value_drops_the_key() {
    let mut idx = hash_index(&[("only", 1)]);
    assert!(idx.delete(&pv("only"), &node(1)));
    assert!(idx.is_empty());
    assert_eq!(idx.lookup(&pv("only")), Vec::<NodeId>::new());
    assert_eq!(idx.entry_count(), 0);
}

#[test]
fn lookup_of_absent_key_is_empty() {
    let idx = hash_index(&[("present", 1)]);
    assert_eq!(idx.lookup(&pv("absent")), Vec::<NodeId>::new());
}

// --- range_scan is declined, not faked --------------------------------------

#[test]
fn hash_index_declines_range_scan() {
    // An index with no key order returns an explicit error rather than
    // pretending — exactly the behaviour the fallible trait method enables.
    let idx = hash_index(&[("a", 1), ("b", 2)]);
    assert_eq!(
        idx.range_scan(&KeyRange::all()),
        Err(IndexError::RangeUnsupported)
    );
    assert_eq!(
        idx.range_scan(&KeyRange::half_open(pv("a"), pv("z"))),
        Err(IndexError::RangeUnsupported)
    );
}

// --- generic over key/value types (no text / NodeId baked in) ---------------

#[test]
fn hash_index_is_generic_over_key_and_value_types() {
    // Integer keys, edge-id-like u64 values — the type bakes in neither text
    // nor NodeId, and needs no Ord (only Eq + Hash) on its keys.
    let mut idx: HashIndex<i64, u64> = HashIndex::new();
    idx.insert(10, 100);
    idx.insert(20, 200);
    idx.insert(10, 101);
    assert_eq!(idx.lookup(&10), vec![100, 101]);
    assert_eq!(idx.lookup(&20), vec![200]);
    assert_eq!(idx.entry_count(), 3);
    assert_eq!(
        idx.range_scan(&KeyRange::all()),
        Err(IndexError::RangeUnsupported)
    );
}

// --- PropertyIndex planner facade: SAME trait-based API as the B-tree -------

#[test]
fn planner_facade_resolves_point_lookup() {
    // The blanket impl makes the HashIndex a PropertyIndex automatically: the
    // planner needs zero knowledge of the concrete type — the same surface it
    // uses for the B-tree (InMemoryIndex).
    let idx = hash_index(&[("alice", 1), ("alice", 2), ("bob", 3)]);
    let facade: &dyn PropertyIndex = &idx;
    assert!(!facade.supports_range());
    assert_eq!(
        facade.probe(&IndexQuery::Equals(qv("alice"))).unwrap(),
        vec![node(1), node(2)]
    );
}

#[test]
fn planner_facade_estimates_selectivity_for_equality() {
    // 1 of 4 nodes matches → selectivity 0.25.
    let idx = hash_index(&[("alice", 1), ("bob", 2), ("carol", 3), ("dave", 4)]);
    let facade: &dyn PropertyIndex = &idx;
    let s = facade.selectivity(&IndexQuery::Equals(qv("alice")));
    assert!((s.fraction() - 0.25).abs() < f64::EPSILON);
    assert!(s.is_at_least_as_selective_as(0.5));
}

#[test]
fn planner_facade_surfaces_range_error() {
    // A *non-empty* equality-only index asked for a range query. This is the
    // case the T-0022 `equality_only` test sketch never covered (it only ever
    // tested an empty index), and it pins the BUG-0020 fix: the facade reports a
    // range query against a range-incapable index as the *least* selective
    // (fraction 1.0) so the planner falls back to a scan — not the *most*
    // selective (0.0), which would make the planner pick an index that then
    // errors on probe.
    let idx = hash_index(&[("a", 1)]);
    let facade: &dyn PropertyIndex = &idx;
    let query = IndexQuery::Range(KeyRange::all());
    // probe surfaces the error explicitly (no silent empty result)...
    assert_eq!(facade.probe(&query), Err(IndexError::RangeUnsupported));
    // ...and selectivity reports the least-selective estimate so the planner
    // falls back to a scan rather than choosing an index that cannot serve it.
    let s = facade.selectivity(&query);
    assert!(
        (s.fraction() - 1.0).abs() < f64::EPSILON,
        "fraction = {}",
        s.fraction()
    );
    assert!(!s.is_at_least_as_selective_as(0.5));
}

#[test]
fn planner_facade_range_on_empty_equality_only_index_is_non_selective() {
    // The empty case must also be non-selective (and not divide by zero).
    let idx: HashIndex<OrderedKey, NodeId> = HashIndex::new();
    let facade: &dyn PropertyIndex = &idx;
    let query = IndexQuery::Range(KeyRange::all());
    assert_eq!(facade.probe(&query), Err(IndexError::RangeUnsupported));
    assert!(!facade.selectivity(&query).is_at_least_as_selective_as(0.5));
}

#[test]
fn planner_can_query_behind_a_boxed_trait_object() {
    // Hold the index type-erased, exactly as the planner's index catalog will —
    // the heterogeneous catalog can mix HashIndex and the B-tree behind one type.
    let boxed: Box<dyn PropertyIndex> = Box::new(hash_index(&[("x", 7)]));
    assert!(!boxed.supports_range());
    assert_eq!(
        boxed.probe(&IndexQuery::Equals(qv("x"))).unwrap(),
        vec![node(7)]
    );
}

// --- a heterogeneous catalog mixes both index types behind one facade -------

#[test]
fn catalog_mixes_btree_and_hash_indices_behind_one_facade() {
    use crate::index::InMemoryIndex;

    // The B-tree (ordered) index on one property...
    let mut btree: InMemoryIndex<OrderedKey, NodeId> = InMemoryIndex::new();
    btree.insert(pv("alice"), node(1));
    // ...and the hash (equality-only) index on another — both held as the SAME
    // type-erased PropertyIndex, the mechanism the planner uses to pick by
    // selectivity without knowing the concrete index type.
    let mut hash: HashIndex<OrderedKey, NodeId> = HashIndex::new();
    hash.insert(pv("blue"), node(2));

    let catalog: Vec<Box<dyn PropertyIndex>> = vec![Box::new(btree), Box::new(hash)];

    // The ordered index advertises range support; the hash one does not.
    assert!(catalog[0].supports_range());
    assert!(!catalog[1].supports_range());

    assert_eq!(
        catalog[0].probe(&IndexQuery::Equals(qv("alice"))).unwrap(),
        vec![node(1)]
    );
    assert_eq!(
        catalog[1].probe(&IndexQuery::Equals(qv("blue"))).unwrap(),
        vec![node(2)]
    );
}

// --- OrderedKey Hash/Eq consistency (load-bearing for HashIndex) ------------
//
// HashIndex keys on `OrderedKey`, whose `Eq` is the cypher_order relation. The
// Hash/Eq contract demands `a == b ⇒ hash(a) == hash(b)`. The non-obvious
// equality class is numeric cross-type (`Integer(1) == Float(1.0)`); if the hash
// disagreed, a HashIndex would store the same key under two buckets and lose
// lookups. These tests pin the contract.

fn hash_of(key: &OrderedKey) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut h);
    h.finish()
}

#[test]
fn equal_keys_hash_equally_integer_and_float() {
    let i = OrderedKey(PropertyValue::Integer(1));
    let f = OrderedKey(PropertyValue::Float(1.0));
    assert_eq!(i, f, "Integer(1) and Float(1.0) are cypher_order-equal");
    assert_eq!(
        hash_of(&i),
        hash_of(&f),
        "equal keys must hash equally (Hash/Eq contract)"
    );
}

#[test]
fn equal_keys_hash_equally_negative_and_positive_zero() {
    let neg = OrderedKey(PropertyValue::Float(-0.0));
    let pos = OrderedKey(PropertyValue::Float(0.0));
    assert_eq!(neg, pos);
    assert_eq!(hash_of(&neg), hash_of(&pos));
}

#[test]
fn all_nans_hash_equally() {
    let a = OrderedKey(PropertyValue::Float(f64::NAN));
    let b = OrderedKey(PropertyValue::Float(-f64::NAN));
    assert_eq!(a, b, "NaNs are cypher_order-equal to each other");
    assert_eq!(hash_of(&a), hash_of(&b));
}

#[test]
fn equal_list_keys_with_mixed_numeric_hash_equally() {
    let a = OrderedKey(PropertyValue::List(vec![
        PropertyValue::Integer(2),
        PropertyValue::String("x".into()),
    ]));
    let b = OrderedKey(PropertyValue::List(vec![
        PropertyValue::Float(2.0),
        PropertyValue::String("x".into()),
    ]));
    assert_eq!(a, b);
    assert_eq!(hash_of(&a), hash_of(&b));
}

#[test]
fn hash_index_finds_cross_type_numeric_key() {
    // Insert under an Integer key, look up with the cypher-equal Float key:
    // the Hash/Eq consistency must make this resolve to the same bucket.
    let mut idx: HashIndex<OrderedKey, NodeId> = HashIndex::new();
    idx.insert(OrderedKey(PropertyValue::Integer(42)), node(1));
    assert_eq!(
        idx.lookup(&OrderedKey(PropertyValue::Float(42.0))),
        vec![node(1)],
        "a Float lookup must find a value stored under the equal Integer key"
    );
    // And it does not double-store: inserting the Float-equal key is a no-op.
    assert!(!idx.insert(OrderedKey(PropertyValue::Float(42.0)), node(1)));
    assert_eq!(idx.entry_count(), 1);
}

#[test]
fn distinct_keys_hash_to_distinct_buckets_in_practice() {
    // Not a contract (collisions are allowed), but a sanity check that the hash
    // is not degenerate: three clearly-different keys land on three values.
    let idx = hash_index(&[("a", 1), ("b", 2), ("c", 3)]);
    assert_eq!(idx.lookup(&pv("a")), vec![node(1)]);
    assert_eq!(idx.lookup(&pv("b")), vec![node(2)]);
    assert_eq!(idx.lookup(&pv("c")), vec![node(3)]);
}

#[test]
fn hash_index_keys_on_every_property_value_variant() {
    // Exercises every arm of the OrderedKey Hash impl (boolean, null, list, map,
    // string, numeric) as a real index key, and that equal keys round-trip while
    // distinct keys stay distinct.
    let mut map = std::collections::BTreeMap::new();
    map.insert("k".to_string(), PropertyValue::Integer(1));

    let keys = [
        PropertyValue::Boolean(true),
        PropertyValue::Boolean(false),
        PropertyValue::Null,
        PropertyValue::String("s".into()),
        PropertyValue::Integer(7),
        PropertyValue::Float(2.5),
        PropertyValue::List(vec![PropertyValue::Boolean(true), PropertyValue::Null]),
        PropertyValue::Map(map),
    ];

    let mut idx: HashIndex<OrderedKey, NodeId> = HashIndex::new();
    for (i, k) in keys.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        idx.insert(OrderedKey(k.clone()), node(i as u64));
    }
    // Every distinct variant is stored under its own key and looks up cleanly.
    assert_eq!(idx.entry_count(), keys.len());
    for (i, k) in keys.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let expected = node(i as u64);
        assert_eq!(idx.lookup(&OrderedKey(k.clone())), vec![expected]);
    }

    // Two structurally-equal maps hash equally and collapse to one entry.
    let mut m1 = std::collections::BTreeMap::new();
    m1.insert("a".to_string(), PropertyValue::Integer(3));
    let mut m2 = std::collections::BTreeMap::new();
    m2.insert("a".to_string(), PropertyValue::Float(3.0)); // Integer(3) == Float(3.0)
    let k1 = OrderedKey(PropertyValue::Map(m1));
    let k2 = OrderedKey(PropertyValue::Map(m2));
    assert_eq!(k1, k2);
    assert_eq!(hash_of(&k1), hash_of(&k2));
}
