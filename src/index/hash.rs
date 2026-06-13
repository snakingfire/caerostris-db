//! [`HashIndex`] — a second, equality-only secondary index type.
//!
//! This is the **extensibility proof** for the pluggable-index interface
//! (T-0025, Cat. 5 = 100): a concrete index type, distinct from the B-tree, that
//! implements the *same* [`SecondaryIndex`](super::SecondaryIndex) trait —
//! **without any change to the trait signature** — and is consultable through
//! the *same* type-erased [`PropertyIndex`](super::PropertyIndex) planner facade
//! the B-tree uses. See `docs/adr/0005-pluggable-index-interface.md`.
//!
//! Where the B-tree reference impl ([`InMemoryIndex`](super::InMemoryIndex)) is
//! an **ordered** multimap keyed on a totally-ordered key, `HashIndex` is an
//! **equality-only** multimap keyed by [`Hash`] + [`Eq`] alone — it has **no key
//! order at all**. That is precisely what makes it the right second type: it
//! cannot fabricate a range scan, so it proves the trait does not bake in the
//! B-tree's ordering assumption. It models a hash index (and is the structural
//! shape a full-text token index would take). Concretely it:
//!
//! - advertises [`IndexCapabilities::equality_only`] —
//!   `supports_range = supports_prefix = false`;
//! - serves point [`lookup`](super::SecondaryIndex::lookup)s in O(1) average;
//! - **declines** [`range_scan`](super::SecondaryIndex::range_scan) with
//!   [`IndexError::RangeUnsupported`] rather than pretending to order keys.
//!
//! Because its key type is bounded by `Eq + Hash` (not `Ord`), a value space
//! with no natural total order — exactly the case the trait was designed to
//! admit — fits this index but could never fit a B-tree.

use std::collections::HashMap;
use std::hash::Hash;

use super::{IndexCapabilities, IndexError, KeyRange, RangeEntries, SecondaryIndex};

/// An in-memory, **equality-only** secondary index: a hash multimap.
///
/// `HashIndex` is the second concrete index type proving the
/// [`SecondaryIndex`](super::SecondaryIndex) interface generalises beyond the
/// B-tree (T-0025). Its keys need only be hashable and equality-comparable
/// (`K: Eq + Hash`) — deliberately **not** [`Ord`] — so key spaces with no total
/// order fit. A key may map to many values (a non-unique secondary index);
/// duplicate `(key, value)` entries are de-duplicated and values under a key are
/// kept in insertion order for deterministic [`lookup`](Self::lookup) results.
///
/// It advertises [`IndexCapabilities::equality_only`] and returns
/// [`IndexError::RangeUnsupported`] from
/// [`range_scan`](SecondaryIndex::range_scan): with no key order there is no
/// well-defined range, so it declines rather than faking one.
///
/// Like [`InMemoryIndex`](super::InMemoryIndex), this is a *reference*
/// implementation for exercising the interface in-memory; the object-store
/// persistence story is owned by the storage layer (SPIKE-0003 / T-0023) and is
/// orthogonal to this contract.
#[derive(Debug, Clone)]
pub struct HashIndex<K, V> {
    entries: HashMap<K, Vec<V>>,
}

impl<K, V> HashIndex<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone + PartialEq,
{
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

// A manual `Default` (rather than `#[derive(Default)]`) so callers get an empty
// index without `K: Default` / `V: Default` bounds the derive would demand.
impl<K, V> Default for HashIndex<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone + PartialEq,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> SecondaryIndex for HashIndex<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone + PartialEq,
{
    type Key = K;
    type Value = V;

    fn capabilities(&self) -> IndexCapabilities {
        IndexCapabilities::equality_only()
    }

    fn insert(&mut self, key: K, value: V) -> bool {
        let bucket = self.entries.entry(key).or_default();
        if bucket.contains(&value) {
            return false;
        }
        bucket.push(value);
        true
    }

    fn delete(&mut self, key: &K, value: &V) -> bool {
        let Some(bucket) = self.entries.get_mut(key) else {
            return false;
        };
        let before = bucket.len();
        bucket.retain(|v| v != value);
        let removed = bucket.len() != before;
        if bucket.is_empty() {
            self.entries.remove(key);
        }
        removed
    }

    fn lookup(&self, key: &K) -> Vec<V> {
        self.entries.get(key).cloned().unwrap_or_default()
    }

    fn range_scan(&self, _range: &KeyRange<K>) -> Result<RangeEntries<K, V>, IndexError> {
        // No key order ⇒ no well-defined range. Decline explicitly rather than
        // pretend — this is the behaviour the fallible trait method exists for,
        // and the proof the trait carries no B-tree ordering assumption.
        Err(IndexError::RangeUnsupported)
    }

    fn entry_count(&self) -> usize {
        self.entries.values().map(Vec::len).sum()
    }
}
