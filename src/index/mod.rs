//! Pluggable secondary indices.
//!
//! Secondary indices are how caerostris-db makes the latency
//! selectivity-envelope theorem *useful in practice*: a selective index on a
//! node property lets the planner anchor an otherwise-unanchored multi-hop
//! `MATCH` to a tiny seed set, keeping bytes-read inside the budget `B_max`
//! (see [`docs/adr/0001-latency-selectivity-envelope.md`] and `EPIC-005`).
//!
//! This module defines the **interface contract** only — the trait every index
//! type implements and the planner-facing facade the planner consults. Concrete
//! on-object-store implementations (the B-tree on text properties, T-0023; an
//! extensibility-proving second type, T-0025) land later, on top of this trait.
//! An [`InMemoryIndex`] reference implementation is provided here so the
//! interface itself is exercised by unit tests without depending on the storage
//! format (SPIKE-0003).
//!
//! # Two layers, deliberately separated
//!
//! 1. [`SecondaryIndex`] — the **generic, fully-typed** trait. Its associated
//!    [`Key`](SecondaryIndex::Key) and [`Value`](SecondaryIndex::Value) types
//!    are parameterised so non-B-tree index shapes fit without core rewrites: a
//!    spatial index keys on a geometry, a full-text index on a token, a
//!    composite index on a tuple. The trait carries **no B-tree-specific
//!    assumptions** — ordering and range support are *advertised*
//!    ([`IndexCapabilities`]) rather than assumed, and
//!    [`range_scan`](SecondaryIndex::range_scan) is fallible so an
//!    equality-only index (hash, full-text) can decline it
//!    ([`IndexError::RangeUnsupported`]) instead of being forced to pretend.
//!
//! 2. [`PropertyIndex`] — the **object-safe, type-erased planner facade**. The
//!    planner holds heterogeneous indices behind `&dyn PropertyIndex` and
//!    chooses among them **by selectivity** without knowing any concrete index
//!    type or its associated types. A [blanket impl](PropertyIndex#impl-PropertyIndex-for-I)
//!    bridges every `SecondaryIndex<Key = PropertyValue, Value = NodeId>` into a
//!    `PropertyIndex` automatically, so a new index type gains the planner facade
//!    for free.
//!
//! See `docs/adr/0004-pluggable-index-interface.md` for the rationale, the
//! rejected alternatives, and the rubric impact.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::ops::Bound;

use crate::model::{NodeId, PropertyValue};

/// A [`PropertyValue`] wrapped so it can serve as an **ordered index key**.
///
/// [`PropertyValue`] intentionally implements neither [`Ord`] nor [`Eq`] — it
/// holds `f64`, which is only [`PartialOrd`], and openCypher distinguishes
/// *value equality* from *structural identity* ([`PropertyValue::cypher_equal`]
/// vs [`PartialEq`]). An ordered index, however, needs a **total order** over
/// its keys. `OrderedKey` provides exactly that by delegating to
/// [`PropertyValue::cypher_order`] — the openCypher orderability relation, which
/// is total over every value and type (NaN sorts greatest, `null` sorts last).
///
/// Its [`Eq`] agrees with that total order (`a == b` iff `cypher_order` is
/// `Equal`), so it is a consistent `Ord`/`Eq` pair safe to use as a
/// [`BTreeMap`] key. This is the key type concrete property→node indices use
/// (e.g. [`InMemoryIndex<OrderedKey, NodeId>`](InMemoryIndex)); the planner-facing
/// [`IndexQuery`] / [`PropertyIndex`] facade speaks plain [`PropertyValue`] and
/// wraps into `OrderedKey` internally, so callers never see it unless they want
/// to.
#[derive(Debug, Clone)]
pub struct OrderedKey(pub PropertyValue);

impl OrderedKey {
    /// The wrapped [`PropertyValue`].
    #[must_use]
    pub fn into_inner(self) -> PropertyValue {
        self.0
    }
}

impl From<PropertyValue> for OrderedKey {
    fn from(v: PropertyValue) -> Self {
        OrderedKey(v)
    }
}

impl From<OrderedKey> for PropertyValue {
    fn from(k: OrderedKey) -> Self {
        k.0
    }
}

impl PartialEq for OrderedKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.cypher_order(&other.0) == Ordering::Equal
    }
}

impl Eq for OrderedKey {}

impl PartialOrd for OrderedKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cypher_order(&other.0)
    }
}

/// Errors an index operation can return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexError {
    /// [`range_scan`](SecondaryIndex::range_scan) was called on an index that
    /// does not support ordered range queries (e.g. a hash or full-text index).
    /// Callers should consult [`IndexCapabilities::supports_range`] first; the
    /// planner does this via [`PropertyIndex::supports_range`].
    RangeUnsupported,
    /// The query shape is not one this index can serve (e.g. a prefix query on a
    /// numeric index). Carries a human-readable reason.
    UnsupportedQuery(String),
}

impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexError::RangeUnsupported => {
                write!(f, "this index does not support ordered range scans")
            }
            IndexError::UnsupportedQuery(why) => {
                write!(f, "index cannot serve this query: {why}")
            }
        }
    }
}

impl std::error::Error for IndexError {}

/// What an index *can do*, advertised so callers never assume B-tree semantics.
///
/// The planner reads these to decide whether a given index can serve a query
/// shape before attempting it. A future full-text index, for instance, reports
/// `supports_range: false, supports_prefix: true`; a hash/equality index reports
/// `false, false`; the B-tree reports `true, true`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexCapabilities {
    /// Whether [`range_scan`](SecondaryIndex::range_scan) returns ordered
    /// results over a key range. `false` for hash/equality-only indices.
    pub supports_range: bool,
    /// Whether the index can answer ordered prefix / "starts-with" style
    /// queries efficiently (a B-tree on text can; a hash index cannot).
    pub supports_prefix: bool,
}

impl IndexCapabilities {
    /// Capabilities of an ordered index (B-tree-like): range and prefix queries.
    #[must_use]
    pub const fn ordered() -> Self {
        Self {
            supports_range: true,
            supports_prefix: true,
        }
    }

    /// Capabilities of an equality-only index (hash / full-text token): point
    /// lookups only, no ordered range or prefix scans.
    #[must_use]
    pub const fn equality_only() -> Self {
        Self {
            supports_range: false,
            supports_prefix: false,
        }
    }
}

/// A half-open or closed key range for [`range_scan`](SecondaryIndex::range_scan).
///
/// A concrete struct (rather than a generic `R: RangeBounds<Key>` parameter) so
/// the trait method stays **non-generic** and the trait stays object-safe — the
/// planner can hold an index behind `&dyn`. Mirrors [`std::ops::RangeBounds`]:
/// use [`Bound::Unbounded`] on either end for an open range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyRange<K> {
    /// Lower bound (inclusive/exclusive/unbounded).
    pub start: Bound<K>,
    /// Upper bound (inclusive/exclusive/unbounded).
    pub end: Bound<K>,
}

impl<K> KeyRange<K> {
    /// `start..end` — inclusive lower bound, exclusive upper bound.
    #[must_use]
    pub fn half_open(start: K, end: K) -> Self {
        Self {
            start: Bound::Included(start),
            end: Bound::Excluded(end),
        }
    }

    /// `start..` — everything from `start` (inclusive) upward.
    #[must_use]
    pub fn from(start: K) -> Self {
        Self {
            start: Bound::Included(start),
            end: Bound::Unbounded,
        }
    }

    /// `..end` — everything below `end` (exclusive).
    #[must_use]
    pub fn until(end: K) -> Self {
        Self {
            start: Bound::Unbounded,
            end: Bound::Excluded(end),
        }
    }

    /// `..` — the whole key space.
    #[must_use]
    pub fn all() -> Self {
        Self {
            start: Bound::Unbounded,
            end: Bound::Unbounded,
        }
    }
}

impl<K> std::ops::RangeBounds<K> for KeyRange<K> {
    fn start_bound(&self) -> Bound<&K> {
        match &self.start {
            Bound::Included(k) => Bound::Included(k),
            Bound::Excluded(k) => Bound::Excluded(k),
            Bound::Unbounded => Bound::Unbounded,
        }
    }

    fn end_bound(&self) -> Bound<&K> {
        match &self.end {
            Bound::Included(k) => Bound::Included(k),
            Bound::Excluded(k) => Bound::Excluded(k),
            Bound::Unbounded => Bound::Unbounded,
        }
    }
}

/// The result of a [`range_scan`](SecondaryIndex::range_scan): the matched
/// `(key, value)` entries in ascending key order. A named alias so the trait
/// signature stays legible (and clippy-clean).
pub type RangeEntries<K, V> = Vec<(K, V)>;

/// A pluggable secondary index over `(Key, Value)` entries.
///
/// Every index type — B-tree on text, range, full-text, spatial, composite —
/// implements this single trait. The associated types keep the contract free of
/// any concrete-index assumptions:
///
/// - [`Key`](Self::Key) is the indexed value. The trait bounds it only by
///   [`Clone`] — **not** [`Ord`] — deliberately: ordering is a *capability* an
///   index advertises ([`IndexCapabilities`]), not a property every key type
///   must have. A B-tree on text orders its keys; a hash or full-text index does
///   not, and a spatial key may have no natural total order at all. An index
///   that *does* order its keys (like [`InMemoryIndex`]) places the ordering
///   bound on its own type parameter, where it belongs, not on the contract.
/// - [`Value`](Self::Value) is what a key maps to (a [`NodeId`], an edge id, a
///   composite locator). Only [`Clone`] is required so lookups can hand back
///   owned results.
///
/// # Multi-valued by design
///
/// A key may map to **many** values (many nodes can share a property value), so
/// [`lookup`](Self::lookup) returns a `Vec` and [`insert`](Self::insert) /
/// [`delete`](Self::delete) operate on a single `(key, value)` entry. This
/// matches a real secondary index on a non-unique property.
///
/// # Capability advertisement, not assumption
///
/// [`capabilities`](Self::capabilities) tells callers what the index can do.
/// [`range_scan`](Self::range_scan) is fallible precisely so an index that does
/// not order its keys ([`IndexCapabilities::supports_range`] = `false`) can
/// return [`IndexError::RangeUnsupported`] rather than being forced into a
/// B-tree shape it cannot honour.
pub trait SecondaryIndex {
    /// The indexed key type. Bounded only by [`Clone`] so non-ordered key types
    /// (full-text tokens, spatial keys) fit; ordered indices add their own
    /// [`Ord`] bound on their concrete key parameter.
    type Key: Clone;
    /// The value a key maps to (e.g. a [`NodeId`]).
    type Value: Clone;

    /// What this index can do. Callers consult this before issuing range or
    /// prefix queries.
    fn capabilities(&self) -> IndexCapabilities;

    /// Insert the `(key, value)` entry. Inserting a `(key, value)` that already
    /// exists is a no-op (entries are a set per key); returns `true` iff the
    /// entry was newly added.
    fn insert(&mut self, key: Self::Key, value: Self::Value) -> bool;

    /// Remove the `(key, value)` entry. Returns `true` iff an entry was removed.
    fn delete(&mut self, key: &Self::Key, value: &Self::Value) -> bool;

    /// Point lookup: every value associated with `key`, in deterministic order.
    /// An absent key yields an empty `Vec`.
    fn lookup(&self, key: &Self::Key) -> Vec<Self::Value>;

    /// Ordered range scan: every `(key, value)` whose key falls in `range`, in
    /// ascending key order.
    ///
    /// # Errors
    ///
    /// Returns [`IndexError::RangeUnsupported`] if this index does not order its
    /// keys (check [`capabilities`](Self::capabilities) first).
    fn range_scan(
        &self,
        range: &KeyRange<Self::Key>,
    ) -> Result<RangeEntries<Self::Key, Self::Value>, IndexError>;

    /// Total number of `(key, value)` entries in the index. Used by the planner
    /// (via the [`PropertyIndex`] facade) to turn a match count into a
    /// selectivity fraction.
    fn entry_count(&self) -> usize;

    /// `true` if the index holds no entries.
    fn is_empty(&self) -> bool {
        self.entry_count() == 0
    }
}

/// A query the planner can ask an index to serve, expressed over
/// [`PropertyValue`]s so it is independent of the concrete index type.
#[derive(Debug, Clone, PartialEq)]
pub enum IndexQuery {
    /// Equality: `WHERE n.prop = <value>`.
    Equals(PropertyValue),
    /// Half-open / open range: `WHERE n.prop >= lo AND n.prop < hi` and the like.
    Range(KeyRange<PropertyValue>),
}

/// An estimate of how selective a query is against an index — the fraction of
/// entries it is expected to match, in `[0.0, 1.0]`.
///
/// The planner compares this against a threshold (and against the cost of a full
/// scan) to decide whether to use the index. **Lower is more selective** (better
/// for anchoring the latency envelope). This is the only selectivity surface the
/// planner needs; it never inspects the concrete index.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Selectivity(f64);

impl Selectivity {
    /// Build a selectivity from a `matched / total` fraction, clamped to
    /// `[0.0, 1.0]`. A `total` of zero (empty index) yields the least-selective
    /// value `1.0`, so the planner conservatively prefers a scan over an empty
    /// index rather than dividing by zero.
    #[must_use]
    pub fn from_fraction(matched: usize, total: usize) -> Self {
        if total == 0 {
            return Self(1.0);
        }
        #[allow(clippy::cast_precision_loss)]
        let f = matched as f64 / total as f64;
        Self(f.clamp(0.0, 1.0))
    }

    /// The raw fraction in `[0.0, 1.0]`.
    #[must_use]
    pub fn fraction(self) -> f64 {
        self.0
    }

    /// `true` if this query is at least as selective as `threshold` (i.e. its
    /// fraction is `<=` the threshold), the test the planner uses to decide an
    /// index is worth using over a full scan.
    #[must_use]
    pub fn is_at_least_as_selective_as(self, threshold: f64) -> bool {
        self.0 <= threshold
    }
}

/// The object-safe, type-erased planner facade over a property→node index.
///
/// The planner consults indices through this trait alone: it estimates
/// [`selectivity`](Self::selectivity), and if an index is selective enough it
/// [`probe`](Self::probe)s for the matching [`NodeId`]s — all **without knowing
/// the concrete index type** or its associated `Key`/`Value`. Any
/// `SecondaryIndex<Key = PropertyValue, Value = NodeId>` is automatically a
/// `PropertyIndex` via the [blanket impl](#impl-PropertyIndex-for-I).
pub trait PropertyIndex {
    /// Whether the underlying index can serve ordered range queries.
    fn supports_range(&self) -> bool;

    /// Estimate the selectivity of `query` — the fraction of indexed nodes it is
    /// expected to match. Used to choose between the index and a full scan.
    fn selectivity(&self, query: &IndexQuery) -> Selectivity;

    /// Resolve `query` to the matching node ids.
    ///
    /// # Errors
    ///
    /// Returns [`IndexError`] if the index cannot serve this query shape (e.g. a
    /// range query against an equality-only index).
    fn probe(&self, query: &IndexQuery) -> Result<Vec<NodeId>, IndexError>;
}

/// Wrap a [`KeyRange<PropertyValue>`] (as carried in an [`IndexQuery`]) into the
/// [`OrderedKey`] domain the concrete index is keyed on.
fn ordered_range(range: &KeyRange<PropertyValue>) -> KeyRange<OrderedKey> {
    fn wrap(b: &Bound<PropertyValue>) -> Bound<OrderedKey> {
        match b {
            Bound::Included(v) => Bound::Included(OrderedKey(v.clone())),
            Bound::Excluded(v) => Bound::Excluded(OrderedKey(v.clone())),
            Bound::Unbounded => Bound::Unbounded,
        }
    }
    KeyRange {
        start: wrap(&range.start),
        end: wrap(&range.end),
    }
}

/// Blanket bridge: every property→node [`SecondaryIndex`] keyed on [`OrderedKey`]
/// is a [`PropertyIndex`]. This is what lets the planner stay ignorant of
/// concrete index types — implement [`SecondaryIndex`] over `OrderedKey`→`NodeId`
/// and the planner facade comes free. The facade speaks plain [`PropertyValue`];
/// the wrapping into [`OrderedKey`] happens here, once.
impl<I> PropertyIndex for I
where
    I: SecondaryIndex<Key = OrderedKey, Value = NodeId>,
{
    fn supports_range(&self) -> bool {
        self.capabilities().supports_range
    }

    fn selectivity(&self, query: &IndexQuery) -> Selectivity {
        let total = self.entry_count();
        let matched = match query {
            IndexQuery::Equals(v) => self.lookup(&OrderedKey(v.clone())).len(),
            IndexQuery::Range(range) => match self.range_scan(&ordered_range(range)) {
                Ok(hits) => hits.len(),
                // An index that cannot range-scan matches nothing for a range
                // query; the planner sees a 0-match (non-selective) estimate and
                // falls back to a scan, while `probe` surfaces the error
                // explicitly rather than silently returning no rows.
                Err(_) => return Selectivity::from_fraction(0, total),
            },
        };
        Selectivity::from_fraction(matched, total)
    }

    fn probe(&self, query: &IndexQuery) -> Result<Vec<NodeId>, IndexError> {
        match query {
            IndexQuery::Equals(v) => Ok(self.lookup(&OrderedKey(v.clone()))),
            IndexQuery::Range(range) => {
                let hits = self.range_scan(&ordered_range(range))?;
                Ok(hits.into_iter().map(|(_, node)| node).collect())
            }
        }
    }
}

/// An in-memory reference [`SecondaryIndex`], generic over key and value.
///
/// This exists to **exercise the interface** in unit tests — it is *not* the
/// production object-store index (that is T-0023). It is an ordered multimap
/// (`BTreeMap<Key, Vec<Value>>`) and therefore advertises
/// [`IndexCapabilities::ordered`], so it can serve both point lookups and range
/// scans. Values under a key are de-duplicated by structural equality and kept
/// in insertion order for deterministic results.
#[derive(Debug, Clone, Default)]
pub struct InMemoryIndex<K: Ord + Clone, V: Clone + PartialEq> {
    entries: BTreeMap<K, Vec<V>>,
}

impl<K: Ord + Clone, V: Clone + PartialEq> InMemoryIndex<K, V> {
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

impl<K: Ord + Clone, V: Clone + PartialEq> SecondaryIndex for InMemoryIndex<K, V> {
    type Key = K;
    type Value = V;

    fn capabilities(&self) -> IndexCapabilities {
        IndexCapabilities::ordered()
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

    fn range_scan(&self, range: &KeyRange<K>) -> Result<RangeEntries<K, V>, IndexError> {
        let mut out = Vec::new();
        for (k, bucket) in self.entries.range((range.start.clone(), range.end.clone())) {
            for v in bucket {
                out.push((k.clone(), v.clone()));
            }
        }
        Ok(out)
    }

    fn entry_count(&self) -> usize {
        self.entries.values().map(Vec::len).sum()
    }
}

#[cfg(test)]
mod tests;
