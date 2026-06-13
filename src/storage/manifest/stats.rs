//! The manifest **statistics block** (SPIKE-0004 / ADR 0008 §5.3).
//!
//! These are the maintained graph statistics the planner reads
//! snapshot-consistently to classify a 6-hop unanchored match as **in-** or
//! **out-of-envelope** before any object-store access (decision 0009 /
//! ADR 0001 §4). Per the inline-vs-referenced cut ratified by `steering-storage`
//! (ADR 0008 §5.3, condition C4 / SPIKE-0004 Part 2.1 (B)):
//!
//! - The **OOE-critical scalars** live **inline** in this block so the
//!   super-hub / non-selective rejection paths need **zero** extra round-trip
//!   beyond resolving the manifest: per-label `node_count`, `total_node_count`,
//!   per-rel-type `edge_count`, `p99_deg`, and the **mandatory `max_deg`**
//!   super-hub safety term (decision 0015 / ADR 0001 F2).
//! - The **bulky per-property selectivity detail** (NDV / null-fraction / MCV /
//!   histogram) is **referenced** as a content-addressed [`StatsBlobRef`]
//!   (`db/stats/<hash>.stats`), fetched lazily during planning only for
//!   properties a query actually filters on. The blob's internal layout is
//!   owned by the storage-format work (T-0007); this module pins only that the
//!   reference is content-addressed and that any value-derived stat is a
//!   **digest, never a raw value** (the value-digest privacy rule).
//!
//! # The value-digest privacy invariant (guardrails §3 / SPIKE-0004 §1.2 / C4)
//!
//! MCV and histogram entries (and column min/max summaries) store a fixed-width
//! collision-resistant **digest** of each value — never the raw property value.
//! This keeps the open-source repo and any committed fixture free of user data
//! by construction. This module's inline block carries no raw values at all
//! (only counts and degrees); [`ValueDigest`] is the type the referenced blob
//! and any future inline summary uses, and it is deliberately a fixed-width byte
//! array with **no** constructor from a raw string/value in this crate.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::ManifestVersion;

/// A fixed-width, collision-resistant digest of a property value (the first 8
/// bytes of a BLAKE3 hash; SPIKE-0004 §1.2). Used wherever a statistic would
/// otherwise need a raw value (MCV entries, histogram boundaries, column
/// min/max) so **no raw user value** is ever stored in a manifest or a
/// committed fixture (guardrails §3, condition C4).
///
/// It is intentionally an opaque fixed-width array: the planner only ever needs
/// to compare a query literal's digest for equality (and, for ordered
/// histograms, an order-preserving truncated key — a separate concern owned by
/// the referenced blob), so the raw value is never required here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ValueDigest(pub [u8; 8]);

impl ValueDigest {
    /// The raw 8 digest bytes.
    #[must_use]
    pub fn bytes(self) -> [u8; 8] {
        self.0
    }
}

/// The freshness / trust marker for a statistic family (SPIKE-0004 §1.4).
///
/// It drives the planner's missing/stale rule (SPIKE-0004 §3.3): the planner
/// **never** makes an optimistic assumption from a statistic it does not trust.
/// `Absent` ⇒ assume the worst case (selectivity 1 / `max_deg = ∞`) ⇒ default to
/// reject/warn. This type carries the marker; the planner (T-0015) applies the
/// rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    /// Maintained exactly as of `as_of_version` (the exact counts).
    Exact,
    /// A bounded estimate, recomputed at the last `ANALYZE` and still trusted.
    Estimated,
    /// Carried forward across enough drift that the planner should degrade to
    /// conservative bounds (SPIKE-0004 §3.3).
    Stale,
    /// No estimator computed yet (e.g. freshly-ingested data). The planner
    /// assumes the worst case. This is the safe default for a brand-new family.
    #[default]
    Absent,
}

/// A relationship-traversal direction (ADR 0008 §3.1: both directions are
/// materialised so in-edge traversal is also `r ≤ 1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Out-edges (source → destination).
    Out,
    /// In-edges (destination → source).
    In,
}

/// Per-label node statistics (SPIKE-0004 §1.1). Inline & **exact** as of the
/// committed version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelStats {
    /// Exact number of nodes carrying this label. Upper bound for `est_N_seed`;
    /// also a Cat. 6 fast-`count` source (zero data GETs).
    pub node_count: u64,
}

/// Per-(rel-type, direction) degree statistics (SPIKE-0004 §1.3).
///
/// The two degree terms have **different jobs** (SPIKE-0004 §3.1):
/// `p99_deg` sizes the *typical* per-hop byte cost (acceptance estimate);
/// **`max_deg` is the super-hub safety term** — the only term that makes the
/// super-hub case detectable at plan time (decision 0015 / ADR 0001 F2). A
/// query must clear **both**. `max_deg` is therefore **mandatory**, not
/// optional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DegreeStats {
    /// Exact total edges of this rel-type (Cat. 6 fast-`count` source).
    pub edge_count: u64,
    /// 99th-percentile out/in-degree — the *typical* fan-out term `F_tail`.
    /// Used for the byte **acceptance** estimate only; **forbidden** as the
    /// byte safety bound (it admits the super-hub).
    pub p99_deg: u32,
    /// **Maximum** out/in-degree of any node over this rel-type — the
    /// **mandatory** super-hub safety term (decision 0015 / ADR 0001 F2). Used
    /// for the byte **safety** gate: a query is rejected if a reachable
    /// rel-type's `max_deg` adjacency list alone busts the per-GET byte cap.
    pub max_deg: u32,
}

/// A reference to a content-addressed selectivity blob (`db/stats/<hash>.stats`;
/// SPIKE-0004 Part 2.1 (B) / ADR 0008 §5.3).
///
/// The bulky per-`(label, property)` selectivity detail (NDV, null-fraction,
/// MCV list, histogram) lives in this referenced blob, fetched lazily during
/// planning only for properties a query filters on. The blob is part of the
/// version's reference set (so GC keeps it while the manifest survives) and its
/// key is content-addressed, so it is snapshot-consistent with the pinned
/// version by construction. All value-derived entries inside it are
/// [`ValueDigest`]s, never raw values (condition C4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsBlobRef {
    /// The content-addressed object-store key of the blob.
    pub key: String,
    /// The `(label, property)` pairs whose selectivity detail this blob carries,
    /// so the planner can fetch only the blob(s) covering a filtered property.
    #[serde(default)]
    pub covers: Vec<LabelProperty>,
}

impl StatsBlobRef {
    /// A blob reference covering no specific properties (the minimal form used
    /// when the coverage map is recorded elsewhere or the blob is monolithic).
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            covers: Vec::new(),
        }
    }
}

/// A `(label, property)` coordinate the selectivity statistics are keyed by.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LabelProperty {
    /// The node label.
    pub label: String,
    /// The property key.
    pub property: String,
}

/// Reproducibility / error-bound parameters for the estimators (SPIKE-0004
/// §1.4). Recorded so a reader can reason about error bounds and a re-`ANALYZE`
/// is reproducible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EstimatorParams {
    /// HyperLogLog precision for NDV estimation.
    pub hll_precision: u8,
    /// Number of most-common-value entries kept per `(label, property)`.
    pub mcv_h: u16,
    /// Number of equi-depth histogram buckets per `(label, property)`.
    pub hist_q: u16,
    /// Deterministic sketch seed (so `ANALYZE` is reproducible).
    pub sketch_seed: u64,
}

impl Default for EstimatorParams {
    fn default() -> Self {
        // SPIKE-0004 defaults: precision 14, H=32 MCV, Q=64 histogram buckets.
        Self {
            hll_precision: 14,
            mcv_h: 32,
            hist_q: 64,
            sketch_seed: 0,
        }
    }
}

/// The schema version of the statistics block itself (SPIKE-0004 §1.4), for
/// forward-compatible evolution of the stats layout independent of the manifest
/// `format_version`.
pub const STATS_VERSION: u32 = 1;

/// The manifest's inline statistics block (SPIKE-0004 §1 / ADR 0008 §5.3).
///
/// Carries **only** the OOE-critical inline scalars + block metadata. The bulky
/// per-property selectivity detail is referenced via [`StatsBlobRef`] on the
/// manifest, not embedded here, so resolving the manifest reads the
/// super-hub / non-selective rejection terms with no extra round-trip
/// (acceptance criterion 4 / condition C4). The block carries **no raw property
/// values** — only counts, degrees, and freshness markers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsBlock {
    /// Schema version of this block ([`STATS_VERSION`]).
    pub stats_version: u32,
    /// The committed manifest version these stats describe (== the manifest's
    /// `V`). Snapshot-consistency marker.
    pub as_of_version: ManifestVersion,
    /// Freshness of the *estimated* families (degree summaries / the referenced
    /// selectivity detail). Exact counts are always exact; this marks the
    /// estimated terms (SPIKE-0004 §2.2).
    pub freshness: Freshness,
    /// Estimator reproducibility / error-bound parameters.
    pub estimator_params: EstimatorParams,
    /// Total nodes in the graph (denominator for `s = N_seed / N_total`). Exact.
    pub total_node_count: u64,
    /// Per-label exact node counts (inline; OOE-critical + Cat. 6 fast count).
    #[serde(default)]
    pub label_stats: BTreeMap<String, LabelStats>,
    /// Per-(rel-type, direction) degree summaries (inline; OOE-critical). The
    /// key is `(rel_type, direction)`. **`max_deg` is mandatory** for every
    /// entry.
    #[serde(default)]
    pub degree_stats: BTreeMap<RelDir, DegreeStats>,
}

/// The `(rel-type, direction)` key the degree statistics are keyed by.
///
/// Serialised as a string `"<rel_type>:<out|in>"` so it is a valid JSON object
/// key (JSON map keys must be strings).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RelDir {
    /// The relationship type.
    pub rel_type: String,
    /// The traversal direction.
    pub direction: Direction,
}

impl RelDir {
    /// Construct a `(rel-type, direction)` key.
    pub fn new(rel_type: impl Into<String>, direction: Direction) -> Self {
        Self {
            rel_type: rel_type.into(),
            direction,
        }
    }
}

impl std::fmt::Display for RelDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dir = match self.direction {
            Direction::Out => "out",
            Direction::In => "in",
        };
        write!(f, "{}:{dir}", self.rel_type)
    }
}

impl Serialize for RelDir {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for RelDir {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        // The rel-type may itself contain ':' in pathological schemas, so split
        // on the LAST ':' to recover the direction suffix unambiguously.
        let (rel_type, dir) = s
            .rsplit_once(':')
            .ok_or_else(|| serde::de::Error::custom(format!("malformed RelDir key {s:?}")))?;
        let direction = match dir {
            "out" => Direction::Out,
            "in" => Direction::In,
            other => {
                return Err(serde::de::Error::custom(format!(
                    "unknown direction {other:?} in RelDir key {s:?}"
                )));
            }
        };
        Ok(RelDir {
            rel_type: rel_type.to_string(),
            direction,
        })
    }
}

impl StatsBlock {
    /// An empty, exact statistics block for an empty database at `version`.
    #[must_use]
    pub fn empty(version: ManifestVersion) -> Self {
        Self {
            stats_version: STATS_VERSION,
            as_of_version: version,
            // An empty graph's counts are exact (all zero) — not "absent".
            freshness: Freshness::Exact,
            estimator_params: EstimatorParams::default(),
            total_node_count: 0,
            label_stats: BTreeMap::new(),
            degree_stats: BTreeMap::new(),
        }
    }

    /// Exact node count for a label, or `None` if the label is unknown to this
    /// version (the planner treats unknown as `absent` ⇒ conservative).
    #[must_use]
    pub fn node_count(&self, label: &str) -> Option<u64> {
        self.label_stats.get(label).map(|s| s.node_count)
    }

    /// Degree statistics for a `(rel-type, direction)`, or `None` if absent.
    ///
    /// `None` is the planner's signal to assume `max_deg = ∞` (SPIKE-0004 §3.3)
    /// — i.e. reject the super-hub safety gate unless explicitly overridden.
    #[must_use]
    pub fn degree(&self, rel_type: &str, direction: Direction) -> Option<DegreeStats> {
        self.degree_stats
            .get(&RelDir::new(rel_type, direction))
            .copied()
    }

    /// Set a label's exact node count.
    pub fn set_label_count(&mut self, label: impl Into<String>, node_count: u64) {
        self.label_stats
            .insert(label.into(), LabelStats { node_count });
    }

    /// Set a `(rel-type, direction)`'s degree statistics.
    ///
    /// All three terms (`edge_count`, `p99_deg`, **`max_deg`**) are required —
    /// `max_deg` is the mandatory super-hub safety term (decision 0015).
    pub fn set_degree(
        &mut self,
        rel_type: impl Into<String>,
        direction: Direction,
        stats: DegreeStats,
    ) {
        self.degree_stats
            .insert(RelDir::new(rel_type, direction), stats);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_block_is_exact_and_zeroed() {
        let s = StatsBlock::empty(ManifestVersion(0));
        assert_eq!(s.stats_version, STATS_VERSION);
        assert_eq!(s.as_of_version, ManifestVersion(0));
        assert_eq!(s.freshness, Freshness::Exact);
        assert_eq!(s.total_node_count, 0);
        assert!(s.label_stats.is_empty());
        assert!(s.degree_stats.is_empty());
    }

    #[test]
    fn label_and_degree_accessors() {
        let mut s = StatsBlock::empty(ManifestVersion(3));
        s.total_node_count = 100;
        s.set_label_count("Person", 80);
        s.set_degree(
            "FOLLOWS",
            Direction::Out,
            DegreeStats {
                edge_count: 500,
                p99_deg: 120,
                max_deg: 40_000_000,
            },
        );
        assert_eq!(s.node_count("Person"), Some(80));
        assert_eq!(s.node_count("Robot"), None);
        let d = s.degree("FOLLOWS", Direction::Out).expect("present");
        assert_eq!(d.edge_count, 500);
        assert_eq!(d.p99_deg, 120);
        assert_eq!(d.max_deg, 40_000_000); // the super-hub safety term
        assert_eq!(s.degree("FOLLOWS", Direction::In), None);
        assert_eq!(s.degree("LIKES", Direction::Out), None);
    }

    #[test]
    fn reldir_serialises_as_string_key_and_round_trips() {
        let rd = RelDir::new("FOLLOWS", Direction::Out);
        assert_eq!(rd.to_string(), "FOLLOWS:out");
        let json = serde_json::to_string(&rd).unwrap();
        assert_eq!(json, "\"FOLLOWS:out\"");
        let back: RelDir = serde_json::from_str(&json).unwrap();
        assert_eq!(rd, back);

        let rd_in = RelDir::new("KNOWS", Direction::In);
        let back_in: RelDir =
            serde_json::from_str(&serde_json::to_string(&rd_in).unwrap()).unwrap();
        assert_eq!(rd_in, back_in);
    }

    #[test]
    fn reldir_handles_rel_type_containing_colon() {
        // Split on the LAST ':' so a rel-type with an embedded colon survives.
        let rd = RelDir::new("ns:FOLLOWS", Direction::Out);
        let json = serde_json::to_string(&rd).unwrap();
        let back: RelDir = serde_json::from_str(&json).unwrap();
        assert_eq!(rd, back);
        assert_eq!(back.rel_type, "ns:FOLLOWS");
        assert_eq!(back.direction, Direction::Out);
    }

    #[test]
    fn reldir_rejects_malformed_keys() {
        assert!(serde_json::from_str::<RelDir>("\"FOLLOWS\"").is_err()); // no direction
        assert!(serde_json::from_str::<RelDir>("\"FOLLOWS:sideways\"").is_err());
    }

    #[test]
    fn degree_map_round_trips_through_json() {
        let mut s = StatsBlock::empty(ManifestVersion(1));
        s.set_degree(
            "FOLLOWS",
            Direction::Out,
            DegreeStats {
                edge_count: 10,
                p99_deg: 3,
                max_deg: 7,
            },
        );
        s.set_degree(
            "FOLLOWS",
            Direction::In,
            DegreeStats {
                edge_count: 10,
                p99_deg: 4,
                max_deg: 9,
            },
        );
        let json = serde_json::to_string(&s).unwrap();
        let back: StatsBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
        // Both directions are distinct entries.
        assert_eq!(back.degree("FOLLOWS", Direction::Out).unwrap().max_deg, 7);
        assert_eq!(back.degree("FOLLOWS", Direction::In).unwrap().max_deg, 9);
    }

    #[test]
    fn freshness_default_is_absent() {
        // A freshly-ingested family with no estimator is `absent` ⇒ the planner
        // assumes the worst case (SPIKE-0004 §3.3). The default must be the safe
        // one.
        assert_eq!(Freshness::default(), Freshness::Absent);
    }

    #[test]
    fn estimator_params_defaults_match_spike_0004() {
        let p = EstimatorParams::default();
        assert_eq!(p.hll_precision, 14);
        assert_eq!(p.mcv_h, 32);
        assert_eq!(p.hist_q, 64);
    }

    #[test]
    fn value_digest_is_opaque_fixed_width() {
        let d = ValueDigest([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(d.bytes(), [1, 2, 3, 4, 5, 6, 7, 8]);
        let json = serde_json::to_string(&d).unwrap();
        let back: ValueDigest = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
    }

    #[test]
    fn stats_blob_ref_carries_coverage() {
        let mut b = StatsBlobRef::new("db/stats/abc.stats");
        assert!(b.covers.is_empty());
        b.covers.push(LabelProperty {
            label: "Person".to_string(),
            property: "country".to_string(),
        });
        let json = serde_json::to_string(&b).unwrap();
        let back: StatsBlobRef = serde_json::from_str(&json).unwrap();
        assert_eq!(b, back);
    }
}
