//! Unit tests for the pluggable secondary-index interface.
//!
//! These exercise the **interface contract** through the [`InMemoryIndex`]
//! reference implementation and through the type-erased [`PropertyIndex`]
//! planner facade. The `equality_only` module additionally *sketches a second
//! index type* (an equality-only, unordered multimap) against the same
//! [`SecondaryIndex`] trait — the test that the trait carries no B-tree-specific
//! assumptions (acceptance criterion 4).

use std::ops::Bound;

use super::*;
use crate::model::{NodeId, PropertyValue};

// --- helpers ----------------------------------------------------------------

/// A string-valued, ordered index key.
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

/// A property→node index built from `(value, node)` pairs.
fn text_index(pairs: &[(&str, u64)]) -> InMemoryIndex<OrderedKey, NodeId> {
    let mut idx = InMemoryIndex::new();
    for (v, n) in pairs {
        idx.insert(pv(v), node(*n));
    }
    idx
}

// --- SecondaryIndex: insert / delete / lookup -------------------------------

#[test]
fn new_index_is_empty() {
    let idx: InMemoryIndex<OrderedKey, NodeId> = InMemoryIndex::new();
    assert!(idx.is_empty());
    assert_eq!(idx.entry_count(), 0);
    assert_eq!(idx.lookup(&pv("anything")), Vec::<NodeId>::new());
}

#[test]
fn insert_then_lookup_returns_value() {
    let idx = text_index(&[("alice", 1)]);
    assert_eq!(idx.lookup(&pv("alice")), vec![node(1)]);
    assert_eq!(idx.entry_count(), 1);
    assert!(!idx.is_empty());
}

#[test]
fn key_maps_to_many_values_in_insertion_order() {
    // Many nodes can share a property value — a non-unique secondary index.
    let idx = text_index(&[("smith", 1), ("smith", 2), ("smith", 3)]);
    assert_eq!(idx.lookup(&pv("smith")), vec![node(1), node(2), node(3)]);
    assert_eq!(idx.entry_count(), 3);
}

#[test]
fn insert_is_idempotent_per_entry() {
    let mut idx = InMemoryIndex::new();
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
    let mut idx = text_index(&[("smith", 1), ("smith", 2)]);
    assert!(idx.delete(&pv("smith"), &node(1)));
    assert_eq!(idx.lookup(&pv("smith")), vec![node(2)]);
    assert_eq!(idx.entry_count(), 1);
}

#[test]
fn delete_missing_entry_returns_false() {
    let mut idx = text_index(&[("smith", 1)]);
    assert!(!idx.delete(&pv("smith"), &node(99)), "value not present");
    assert!(!idx.delete(&pv("jones"), &node(1)), "key not present");
    assert_eq!(idx.entry_count(), 1);
}

#[test]
fn deleting_last_value_drops_the_key() {
    let mut idx = text_index(&[("only", 1)]);
    assert!(idx.delete(&pv("only"), &node(1)));
    assert!(idx.is_empty());
    assert_eq!(idx.lookup(&pv("only")), Vec::<NodeId>::new());
}

// --- SecondaryIndex: range_scan ---------------------------------------------

#[test]
fn range_scan_returns_entries_in_key_order() {
    let idx = text_index(&[("c", 3), ("a", 1), ("b", 2)]);
    let hits = idx.range_scan(&KeyRange::all()).unwrap();
    assert_eq!(
        hits,
        vec![(pv("a"), node(1)), (pv("b"), node(2)), (pv("c"), node(3))]
    );
}

#[test]
fn range_scan_half_open_excludes_upper_bound() {
    let idx = text_index(&[("a", 1), ("b", 2), ("c", 3)]);
    // [a, c) → a, b but not c
    let hits = idx
        .range_scan(&KeyRange::half_open(pv("a"), pv("c")))
        .unwrap();
    assert_eq!(hits, vec![(pv("a"), node(1)), (pv("b"), node(2))]);
}

#[test]
fn range_scan_from_is_inclusive_lower_unbounded_upper() {
    let idx = text_index(&[("a", 1), ("b", 2), ("c", 3)]);
    let hits = idx.range_scan(&KeyRange::from(pv("b"))).unwrap();
    assert_eq!(hits, vec![(pv("b"), node(2)), (pv("c"), node(3))]);
}

#[test]
fn range_scan_until_is_exclusive_upper() {
    let idx = text_index(&[("a", 1), ("b", 2), ("c", 3)]);
    let hits = idx.range_scan(&KeyRange::until(pv("b"))).unwrap();
    assert_eq!(hits, vec![(pv("a"), node(1))]);
}

#[test]
fn range_scan_prefix_via_string_upper_bound() {
    // A "starts with 'ba'" query expressed as the half-open range [ba, bb).
    let idx = text_index(&[("bar", 1), ("baz", 2), ("bca", 3), ("alpha", 4)]);
    let hits = idx
        .range_scan(&KeyRange::half_open(pv("ba"), pv("bb")))
        .unwrap();
    assert_eq!(hits, vec![(pv("bar"), node(1)), (pv("baz"), node(2))]);
}

#[test]
fn range_scan_explicit_bounds_inclusive_upper() {
    let idx = text_index(&[("a", 1), ("b", 2), ("c", 3)]);
    let range = KeyRange {
        start: Bound::Excluded(pv("a")),
        end: Bound::Included(pv("c")),
    };
    let hits = idx.range_scan(&range).unwrap();
    assert_eq!(hits, vec![(pv("b"), node(2)), (pv("c"), node(3))]);
}

// --- the index works for non-text keys/values too (no text/B-tree leak) -----

#[test]
fn index_is_generic_over_key_and_value_types() {
    // Integer keys, edge-id-like u64 values — the trait does not bake in text.
    let mut idx: InMemoryIndex<i64, u64> = InMemoryIndex::new();
    idx.insert(10, 100);
    idx.insert(20, 200);
    idx.insert(10, 101);
    assert_eq!(idx.lookup(&10), vec![100, 101]);
    let hits = idx.range_scan(&KeyRange::from(15)).unwrap();
    assert_eq!(hits, vec![(20, 200)]);
}

// --- Selectivity -------------------------------------------------------------

#[test]
fn selectivity_fraction_is_matched_over_total() {
    let s = Selectivity::from_fraction(1, 4);
    assert!((s.fraction() - 0.25).abs() < f64::EPSILON);
}

#[test]
fn selectivity_of_empty_index_is_least_selective() {
    // total == 0 must not divide by zero; conservatively reports 1.0.
    let s = Selectivity::from_fraction(0, 0);
    assert!((s.fraction() - 1.0).abs() < f64::EPSILON);
    assert!(!s.is_at_least_as_selective_as(0.5));
}

#[test]
fn selectivity_is_clamped_to_unit_interval() {
    assert!((Selectivity::from_fraction(10, 2).fraction() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn selectivity_threshold_comparison() {
    let selective = Selectivity::from_fraction(1, 100);
    let unselective = Selectivity::from_fraction(90, 100);
    assert!(selective.is_at_least_as_selective_as(0.1));
    assert!(!unselective.is_at_least_as_selective_as(0.1));
}

// --- PropertyIndex planner facade (type-erased, no concrete type) -----------

/// The planner only ever sees `&dyn PropertyIndex`. This helper proves the facade
/// is object-safe and that callers need no knowledge of the concrete index.
fn choose_seed_set(
    idx: &dyn PropertyIndex,
    query: &IndexQuery,
    threshold: f64,
) -> Option<Vec<NodeId>> {
    if idx
        .selectivity(query)
        .is_at_least_as_selective_as(threshold)
    {
        Some(
            idx.probe(query)
                .expect("selective query should probe cleanly"),
        )
    } else {
        None // planner falls back to a full scan
    }
}

#[test]
fn planner_uses_index_when_query_is_selective() {
    // 1 of 4 nodes matches → selectivity 0.25, under a 0.5 threshold.
    let idx = text_index(&[("alice", 1), ("bob", 2), ("carol", 3), ("dave", 4)]);
    let query = IndexQuery::Equals(qv("alice"));
    let seeds = choose_seed_set(&idx, &query, 0.5);
    assert_eq!(seeds, Some(vec![node(1)]));
}

#[test]
fn planner_falls_back_to_scan_when_query_is_unselective() {
    // 3 of 4 match → 0.75 selectivity, above the 0.5 threshold → no index use.
    let idx = text_index(&[("hot", 1), ("hot", 2), ("hot", 3), ("cold", 4)]);
    let query = IndexQuery::Equals(qv("hot"));
    assert_eq!(choose_seed_set(&idx, &query, 0.5), None);
}

#[test]
fn planner_probe_resolves_range_query_to_node_ids() {
    let idx = text_index(&[("a", 1), ("b", 2), ("c", 3)]);
    let query = IndexQuery::Range(KeyRange::half_open(qv("a"), qv("c")));
    let probed = PropertyIndex::probe(&idx, &query).unwrap();
    assert_eq!(probed, vec![node(1), node(2)]);
}

#[test]
fn planner_can_query_behind_a_boxed_trait_object() {
    // Hold the index type-erased, exactly as the planner's index catalog will.
    let boxed: Box<dyn PropertyIndex> = Box::new(text_index(&[("x", 7)]));
    assert!(boxed.supports_range());
    let probed = boxed.probe(&IndexQuery::Equals(qv("x"))).unwrap();
    assert_eq!(probed, vec![node(7)]);
}

// --- Extensibility proof: a SECOND index type against the same trait --------
//
// This module sketches an equality-only index (a hash multimap) implementing the
// SAME `SecondaryIndex` trait. It is the acceptance-criterion-4 proof that the
// trait carries no B-tree-specific assumptions: an index that *cannot* order its
// keys advertises `supports_range = false` and declines `range_scan` with
// `IndexError::RangeUnsupported`, yet still satisfies the trait and the planner
// facade for point lookups.

mod equality_only {
    use super::*;

    /// A second index type: an equality-only multimap with **no key ordering**.
    ///
    /// It is backed by a flat `Vec` of `(key, value)` pairs and answers lookups
    /// by linear equality match — it never relies on a `BTreeMap` or any ordered
    /// structure, modelling a hash- or token-style index. It implements the same
    /// [`SecondaryIndex`] trait, advertises [`IndexCapabilities::equality_only`],
    /// and **declines** [`range_scan`](SecondaryIndex::range_scan) with
    /// [`IndexError::RangeUnsupported`] rather than pretending to order keys.
    /// This is the proof (acceptance criterion 4) that the trait carries no
    /// B-tree-specific assumptions.
    struct EqualityIndex<K, V> {
        entries: Vec<(K, V)>,
    }

    impl<K: PartialEq + Clone, V: PartialEq + Clone> EqualityIndex<K, V> {
        fn new() -> Self {
            Self {
                entries: Vec::new(),
            }
        }
    }

    impl<K, V> SecondaryIndex for EqualityIndex<K, V>
    where
        K: PartialEq + Clone,
        V: PartialEq + Clone,
    {
        type Key = K;
        type Value = V;

        fn capabilities(&self) -> IndexCapabilities {
            IndexCapabilities::equality_only()
        }

        fn insert(&mut self, key: K, value: V) -> bool {
            if self.entries.iter().any(|(k, v)| *k == key && *v == value) {
                return false;
            }
            self.entries.push((key, value));
            true
        }

        fn delete(&mut self, key: &K, value: &V) -> bool {
            let before = self.entries.len();
            self.entries.retain(|(k, v)| !(k == key && v == value));
            self.entries.len() != before
        }

        fn lookup(&self, key: &K) -> Vec<V> {
            self.entries
                .iter()
                .filter(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
                .collect()
        }

        fn range_scan(&self, _range: &KeyRange<K>) -> Result<RangeEntries<K, V>, IndexError> {
            // No key order — it declines explicitly, it does not pretend.
            Err(IndexError::RangeUnsupported)
        }

        fn entry_count(&self) -> usize {
            self.entries.len()
        }
    }

    #[test]
    fn second_index_type_satisfies_the_trait_for_point_lookups() {
        let mut idx: EqualityIndex<OrderedKey, NodeId> = EqualityIndex::new();
        idx.insert(pv("alice"), node(1));
        idx.insert(pv("alice"), node(2));
        assert_eq!(idx.lookup(&pv("alice")), vec![node(1), node(2)]);
        assert_eq!(idx.entry_count(), 2);
        assert!(idx.delete(&pv("alice"), &node(1)));
        assert_eq!(idx.lookup(&pv("alice")), vec![node(2)]);
    }

    #[test]
    fn second_index_type_advertises_no_range_support() {
        let idx: EqualityIndex<OrderedKey, NodeId> = EqualityIndex::new();
        assert!(!idx.capabilities().supports_range);
        assert!(!idx.capabilities().supports_prefix);
    }

    #[test]
    fn second_index_type_declines_range_scan() {
        let idx: EqualityIndex<OrderedKey, NodeId> = EqualityIndex::new();
        assert_eq!(
            idx.range_scan(&KeyRange::all()),
            Err(IndexError::RangeUnsupported)
        );
    }

    #[test]
    fn second_index_type_works_through_the_planner_facade() {
        // The blanket impl makes the equality index a PropertyIndex
        // automatically — the planner needs zero knowledge of the concrete type.
        let mut idx: EqualityIndex<OrderedKey, NodeId> = EqualityIndex::new();
        idx.insert(pv("alice"), node(1));
        let facade: &dyn PropertyIndex = &idx;
        assert!(!facade.supports_range());
        assert_eq!(
            facade.probe(&IndexQuery::Equals(qv("alice"))).unwrap(),
            vec![node(1)]
        );
    }

    #[test]
    fn planner_facade_surfaces_range_error_on_equality_only_index() {
        let idx: EqualityIndex<OrderedKey, NodeId> = EqualityIndex::new();
        let facade: &dyn PropertyIndex = &idx;
        let query = IndexQuery::Range(KeyRange::all());
        assert_eq!(facade.probe(&query), Err(IndexError::RangeUnsupported));
        // And selectivity does not panic — it reports a non-selective estimate.
        assert!(!facade.selectivity(&query).is_at_least_as_selective_as(0.5));
    }
}

// --- IndexError display ------------------------------------------------------

#[test]
fn index_error_displays_human_readable() {
    assert!(IndexError::RangeUnsupported.to_string().contains("range"));
    assert!(
        IndexError::UnsupportedQuery("bad".into())
            .to_string()
            .contains("bad")
    );
}

// --- OrderedKey: total order over PropertyValue ------------------------------

#[test]
fn ordered_key_sorts_by_cypher_order() {
    use std::cmp::Ordering;
    // String orderability is lexicographic.
    assert_eq!(pv("apple").cmp(&pv("banana")), Ordering::Less);
    assert_eq!(pv("banana").cmp(&pv("apple")), Ordering::Greater);
    assert_eq!(pv("apple").cmp(&pv("apple")), Ordering::Equal);
}

#[test]
fn ordered_key_mixed_numeric_compares_by_value() {
    // Integer(1) and Float(1.0) are equal under openCypher orderability, so as
    // index keys they collapse — exactly what a numeric index needs.
    let i = OrderedKey(PropertyValue::Integer(1));
    let f = OrderedKey(PropertyValue::Float(1.0));
    assert_eq!(i, f);
    assert_eq!(i.cmp(&f), std::cmp::Ordering::Equal);
}

#[test]
fn ordered_key_can_key_a_btree_index_with_numeric_values() {
    // A numeric property index: proves OrderedKey works for non-text keys and
    // that range scans honour numeric order.
    let mut idx: InMemoryIndex<OrderedKey, NodeId> = InMemoryIndex::new();
    idx.insert(OrderedKey(PropertyValue::Integer(30)), node(1));
    idx.insert(OrderedKey(PropertyValue::Integer(10)), node(2));
    idx.insert(OrderedKey(PropertyValue::Integer(20)), node(3));
    let hits = idx
        .range_scan(&KeyRange::from(OrderedKey(PropertyValue::Integer(15))))
        .unwrap();
    let nodes: Vec<NodeId> = hits.into_iter().map(|(_, n)| n).collect();
    // 20 then 30, in numeric (not insertion) order.
    assert_eq!(nodes, vec![node(3), node(1)]);
}

#[test]
fn ordered_key_round_trips_through_property_value() {
    let original = PropertyValue::String("zed".into());
    let key: OrderedKey = original.clone().into();
    let back: PropertyValue = key.clone().into();
    assert_eq!(back, original);
    assert_eq!(key.into_inner(), original);
}

// --- IndexCapabilities constructors -----------------------------------------

#[test]
fn capability_constructors() {
    assert_eq!(
        IndexCapabilities::ordered(),
        IndexCapabilities {
            supports_range: true,
            supports_prefix: true,
        }
    );
    assert_eq!(
        IndexCapabilities::equality_only(),
        IndexCapabilities {
            supports_range: false,
            supports_prefix: false,
        }
    );
}
