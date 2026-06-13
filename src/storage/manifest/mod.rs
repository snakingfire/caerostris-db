//! The manifest: the immutable per-version root object of a caerostris-db
//! database.
//!
//! A **manifest** is the single object a reader resolves on open. It is the
//! root of a committed version `V`: it lists the **exact** set of data-object
//! keys that version references, carries the format version and schema
//! metadata, and embeds the snapshot-consistent **statistics block** the
//! planner reads for out-of-envelope detection. The commit protocol (ADR 0002 /
//! SPIKE-0002) is the *atomic create* of the next manifest object; this module
//! owns the manifest **structure**, its **serialisation**, the **statistics
//! contract** (SPIKE-0004 / ADR 0008 §5), and **latest-version resolution**
//! (ADR 0002 §2.3 / §7.1).
//!
//! It deliberately does **not** own the commit mechanics (create-only CAS,
//! durability barrier, GC) — those belong to T-0010 against ADR 0002 — nor the
//! on-object byte format of data shards (`.ncol` / `.adj`) — those belong to
//! T-0007 / T-0008 against ADR 0008. The manifest references data shards by
//! their **content-addressed key** (a string); it never embeds their bytes.
//!
//! # The pieces
//!
//! - [`Manifest`] — the version root: `format_version`, `manifest_version`,
//!   `created_at`, `schema`, the exact `objects` reference set, the
//!   `partition_map`, the inline `stats` block, and references to bulky
//!   selectivity `stats_blobs`.
//! - [`ObjectRef`] — one entry in the manifest's exact object reference set: a
//!   content-addressed key plus the metadata a reader needs to locate the
//!   object's byte ranges from the manifest alone (ADR 0008 §5).
//! - [`ManifestVersion`] / [`manifest_key`] — the monotone, zero-padded
//!   `db/manifest/<V>.json` key scheme (ADR 0002 §1) whose lexicographic LIST
//!   order equals numeric version order.
//! - [`stats`] — the statistics block (SPIKE-0004 / ADR 0008 §5.3): per-label
//!   counts, per-rel-type degree summaries with the **mandatory `max_deg`**
//!   super-hub safety term, value-digest privacy, freshness markers.
//! - [`resolve`] — latest-version resolution over an
//!   [`ObjectStore`](crate::storage::ObjectStore) and the read of a pinned
//!   manifest plus its referenced objects.
//!
//! All types are serde-(de)serialisable. The manifest is encoded as JSON for
//! debuggability and additive forward-compatibility (ADR 0008 §5).

pub mod resolve;
pub mod stats;

use serde::{Deserialize, Serialize};

pub use resolve::{
    ManifestStoreError, read_manifest, read_referenced_objects, resolve_latest,
    resolve_latest_hint, resolve_latest_version,
};
pub use stats::{DegreeStats, Direction, Freshness, LabelStats, StatsBlobRef, StatsBlock};

/// The S3 key prefix every manifest object lives under (ADR 0002 §1).
pub const MANIFEST_PREFIX: &str = "db/manifest/";

/// The advisory `_latest` pointer key (ADR 0002 §1). It is **never** trusted on
/// its own — resolution always falls back to a `LIST` of [`MANIFEST_PREFIX`].
pub const LATEST_POINTER_KEY: &str = "db/manifest/_latest";

/// Zero-padding width for the version component of a manifest key.
///
/// A `u64` is at most 20 decimal digits, so 20 digits is sufficient for every
/// representable version and makes the **lexicographic** order of manifest keys
/// **identical** to their numeric version order — the property latest-version
/// resolution relies on (ADR 0002 §2.3: "take the max key").
const VERSION_KEY_WIDTH: usize = 20;

/// A committed manifest version number.
///
/// Versions are minted **monotonically** by the single writer (R2): the commit
/// of version `V+1` is the atomic create of `db/manifest/<V+1>.json` (ADR 0002
/// §2). Wrapping the `u64` keeps a version from being silently confused with a
/// node id, an edge count, or any other `u64` the engine passes around.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ManifestVersion(pub u64);

impl ManifestVersion {
    /// The genesis version of an empty, freshly-created database.
    pub const GENESIS: ManifestVersion = ManifestVersion(0);

    /// The raw `u64` behind this version.
    #[must_use]
    pub fn get(self) -> u64 {
        self.0
    }

    /// The next version after this one (the version a commit would mint).
    ///
    /// # Panics
    ///
    /// Panics if this is `u64::MAX` (a version space exhaustion that cannot
    /// occur in any realisable run — `u64` versions at the maximum commit rate
    /// would take longer than the age of the universe).
    #[must_use]
    pub fn next(self) -> ManifestVersion {
        ManifestVersion(
            self.0
                .checked_add(1)
                .expect("manifest version space exhausted (u64::MAX)"),
        )
    }
}

impl std::fmt::Display for ManifestVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The object-store key for a manifest version: `db/manifest/<V>.json`, with
/// `<V>` **zero-padded** to [`VERSION_KEY_WIDTH`] digits.
///
/// Zero-padding makes the lexicographic order of these keys identical to the
/// numeric order of the versions, so `max(LIST db/manifest/)` resolves to the
/// latest committed version (ADR 0002 §2.3).
///
/// ```
/// use caerostris_db::storage::manifest::{manifest_key, ManifestVersion};
///
/// assert_eq!(manifest_key(ManifestVersion(0)),  "db/manifest/00000000000000000000.json");
/// assert_eq!(manifest_key(ManifestVersion(42)), "db/manifest/00000000000000000042.json");
/// // Lexicographic order == numeric order:
/// assert!(manifest_key(ManifestVersion(9)) < manifest_key(ManifestVersion(10)));
/// ```
#[must_use]
pub fn manifest_key(version: ManifestVersion) -> String {
    format!(
        "{MANIFEST_PREFIX}{:0width$}.json",
        version.0,
        width = VERSION_KEY_WIDTH
    )
}

/// Parse a manifest version out of a `db/manifest/<V>.json` key.
///
/// Returns `None` for any key that is not a well-formed manifest object key —
/// including the advisory [`LATEST_POINTER_KEY`] (`_latest`), so the resolver
/// can safely ignore it when scanning the `LIST` result.
///
/// ```
/// use caerostris_db::storage::manifest::{parse_manifest_key, ManifestVersion};
///
/// assert_eq!(
///     parse_manifest_key("db/manifest/00000000000000000042.json"),
///     Some(ManifestVersion(42))
/// );
/// assert_eq!(parse_manifest_key("db/manifest/_latest"), None);
/// assert_eq!(parse_manifest_key("db/data/abc/shard.ncol"), None);
/// ```
#[must_use]
pub fn parse_manifest_key(key: &str) -> Option<ManifestVersion> {
    let rest = key.strip_prefix(MANIFEST_PREFIX)?;
    let digits = rest.strip_suffix(".json")?;
    // A well-formed manifest key is exactly VERSION_KEY_WIDTH ASCII digits.
    // Reject anything else (the `_latest` pointer, truncated keys, foreign
    // objects) rather than silently mis-parsing.
    if digits.len() != VERSION_KEY_WIDTH || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u64>().ok().map(ManifestVersion)
}

/// The kind of object a [`ObjectRef`] points at (ADR 0008 §1 key families).
///
/// The manifest records the kind so a reader can route an object to the right
/// decoder without parsing its bytes first. Unknown kinds are preserved on
/// round-trip via [`ObjectKind::Other`] so an older reader fails *closed* rather
/// than dropping a referenced object from the enumeration (ADR 0008 §8.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    /// Columnar node-property shard (`.ncol`; ADR 0008 §2).
    Ncol,
    /// CSR adjacency shard (`.adj`; ADR 0008 §3).
    Adj,
    /// Secondary-index shard (`.idx`; ADR 0005 / ADR 0008 §5.2).
    Idx,
    /// A kind this build does not recognise. Preserved verbatim so the object
    /// is still enumerated (durability barrier) even though its bytes cannot be
    /// decoded by this reader.
    #[serde(untagged)]
    Other(String),
}

/// One entry in a manifest's **exact, complete** object reference set
/// (ADR 0008 §5; ADR 0002 §7.1 "format obligation").
///
/// Every data/index/stats object a version depends on appears here. A reader
/// resolving the manifest can therefore enumerate and read **every** object the
/// version references (the durability-barrier invariant, SPIKE-0005 Constraint
/// 3), and GC can decide an object is collectible iff it appears in **no**
/// surviving manifest's reference set (ADR 0008 §7.2 / decision 0027 BC-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectRef {
    /// The content-addressed object-store key (e.g.
    /// `db/data/<blake3>/nodes-Person-000123.ncol`). Unique per distinct write
    /// (ADR 0002 §1) — the manifest stores the key, never the bytes.
    pub key: String,
    /// What kind of object this is.
    pub kind: ObjectKind,
}

impl ObjectRef {
    /// Construct an object reference.
    pub fn new(key: impl Into<String>, kind: ObjectKind) -> Self {
        Self {
            key: key.into(),
            kind,
        }
    }
}

/// Schema metadata carried inline in the manifest (ADR 0008 §5).
///
/// This is the name registry (labels / rel-types / property keys) plus the
/// per-rel-type `colocated_projection` flag that drives the explicit `K_min`
/// fallback (ADR 0008 §7.3): when a rel-type's projection set is too wide to
/// co-locate, the planner re-pins the byte budget at `K_min = 9` for plans over
/// it — recorded here, **never silent** (condition C6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaMeta {
    /// Known node labels.
    #[serde(default)]
    pub labels: Vec<String>,
    /// Known relationship types.
    #[serde(default)]
    pub rel_types: Vec<String>,
    /// Known property keys.
    #[serde(default)]
    pub property_keys: Vec<String>,
    /// Per-rel-type: whether the returnable/filterable projection set is
    /// co-located in the adjacency neighbour block (the default, keeping
    /// `K_min = 8`). A rel-type absent from this map is treated as co-located.
    /// A rel-type mapped to `false` forces the explicit `K_min = 9` fallback
    /// for plans over it (ADR 0008 §7.3, condition C6).
    #[serde(default)]
    pub colocated_projection: std::collections::BTreeMap<String, bool>,
}

impl SchemaMeta {
    /// Whether plans over `rel_type` may use the default co-located `K_min = 8`
    /// access pattern. Defaults to `true` (co-located) for any rel-type not
    /// explicitly marked.
    #[must_use]
    pub fn is_colocated(&self, rel_type: &str) -> bool {
        self.colocated_projection
            .get(rel_type)
            .copied()
            .unwrap_or(true)
    }
}

/// The on-disk format version of the manifest JSON itself (ADR 0008 §8.2).
///
/// A reader **fails closed** on a `format_version` it does not understand
/// rather than mis-reading additive fields (BUG-0014 lesson; ADR 0008 §8.2).
pub const MANIFEST_FORMAT_VERSION: u32 = 1;

/// The immutable per-version root object of a database (ADR 0008 §5).
///
/// A `Manifest` is created **exactly once** per version (ADR 0002 §1) and is
/// the only thing that makes a version's data objects reachable. Resolving it
/// yields a complete, consistent snapshot: the exact `objects` reference set,
/// the schema, and the inline statistics — all read in the single round-trip
/// that resolves the version (SPIKE-0004 §2.1, condition C4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// The manifest JSON format version (ADR 0008 §8.2). A reader rejects a
    /// version it does not understand (fail-closed).
    pub format_version: u32,
    /// This manifest's version `V` (ADR 0002 monotone). Equals the version
    /// encoded in its [`manifest_key`].
    pub manifest_version: ManifestVersion,
    /// RFC 3339 creation timestamp (advisory; for debuggability/recovery).
    pub created_at: String,
    /// Schema name registry + projection-co-location flags.
    pub schema: SchemaMeta,
    /// The **exact, complete** set of objects this version references
    /// (ADR 0002 §7.1). A reader can enumerate and read every one
    /// (durability-barrier invariant); GC reclaims an object iff it is in no
    /// surviving manifest's reference set.
    pub objects: Vec<ObjectRef>,
    /// The inline statistics block (OOE-critical scalars; SPIKE-0004 / ADR 0008
    /// §5.3). Readable with **zero** extra round-trip beyond resolving the
    /// manifest (acceptance criterion 4).
    pub stats: StatsBlock,
    /// References to bulky per-property selectivity blobs (`db/stats/<hash>`),
    /// fetched lazily during planning only for filtered properties (SPIKE-0004
    /// Part 2.1 (B) hybrid; ADR 0008 §5.3). These are also part of the version's
    /// reference set for GC purposes (see [`Manifest::referenced_keys`]).
    #[serde(default)]
    pub stats_blobs: Vec<StatsBlobRef>,
}

impl Manifest {
    /// Construct an empty genesis manifest for a freshly-created database.
    ///
    /// It references no data objects and carries an empty, exact statistics
    /// block (a brand-new database has exact zero counts).
    #[must_use]
    pub fn genesis(created_at: impl Into<String>) -> Self {
        Self {
            format_version: MANIFEST_FORMAT_VERSION,
            manifest_version: ManifestVersion::GENESIS,
            created_at: created_at.into(),
            schema: SchemaMeta::default(),
            objects: Vec::new(),
            stats: StatsBlock::empty(ManifestVersion::GENESIS),
            stats_blobs: Vec::new(),
        }
    }

    /// This manifest's object-store key (`db/manifest/<V>.json`).
    #[must_use]
    pub fn key(&self) -> String {
        manifest_key(self.manifest_version)
    }

    /// Serialise the manifest to its canonical JSON bytes.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`serde_json::Error`] if serialisation fails
    /// (which, for this always-serialisable type, indicates a programmer error
    /// such as a non-finite float in a future field).
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec_pretty(self)
    }

    /// Parse a manifest from its JSON bytes, **failing closed** on a
    /// `format_version` this build does not understand (ADR 0008 §8.2).
    ///
    /// # Errors
    ///
    /// - [`ManifestParseError::Json`] if the bytes are not valid manifest JSON.
    /// - [`ManifestParseError::UnsupportedFormatVersion`] if the manifest's
    ///   `format_version` exceeds [`MANIFEST_FORMAT_VERSION`] — a reader never
    ///   mis-reads a newer manifest's additive fields.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ManifestParseError> {
        let manifest: Manifest = serde_json::from_slice(bytes)?;
        if manifest.format_version > MANIFEST_FORMAT_VERSION {
            return Err(ManifestParseError::UnsupportedFormatVersion {
                found: manifest.format_version,
                supported: MANIFEST_FORMAT_VERSION,
            });
        }
        Ok(manifest)
    }

    /// Every object-store key this version references — data/index objects
    /// **and** referenced statistics blobs.
    ///
    /// This is the **complete reference set** for the version (ADR 0002 §7.1):
    /// the keys a reader must be able to read (durability-barrier invariant,
    /// SPIKE-0005 Constraint 3) and the keys GC must treat as live while this
    /// manifest survives (ADR 0008 §7.2, condition C5).
    #[must_use]
    pub fn referenced_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.objects.iter().map(|o| o.key.clone()).collect();
        keys.extend(self.stats_blobs.iter().map(|b| b.key.clone()));
        keys
    }
}

/// Errors from parsing a manifest's bytes.
#[derive(Debug)]
pub enum ManifestParseError {
    /// The bytes are not valid manifest JSON.
    Json(serde_json::Error),
    /// The manifest declares a `format_version` newer than this build supports.
    /// The reader fails **closed** (ADR 0008 §8.2) rather than mis-reading.
    UnsupportedFormatVersion {
        /// The version found in the manifest.
        found: u32,
        /// The newest version this build supports.
        supported: u32,
    },
}

impl std::fmt::Display for ManifestParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestParseError::Json(e) => write!(f, "invalid manifest JSON: {e}"),
            ManifestParseError::UnsupportedFormatVersion { found, supported } => write!(
                f,
                "manifest format_version {found} is newer than supported {supported}; \
                 refusing to read (fail-closed)"
            ),
        }
    }
}

impl std::error::Error for ManifestParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ManifestParseError::Json(e) => Some(e),
            ManifestParseError::UnsupportedFormatVersion { .. } => None,
        }
    }
}

impl From<serde_json::Error> for ManifestParseError {
    fn from(e: serde_json::Error) -> Self {
        ManifestParseError::Json(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_key_is_zero_padded_and_lex_sorts_numerically() {
        assert_eq!(
            manifest_key(ManifestVersion(0)),
            "db/manifest/00000000000000000000.json"
        );
        assert_eq!(
            manifest_key(ManifestVersion(42)),
            "db/manifest/00000000000000000042.json"
        );
        // The decisive property for max-by-LIST resolution: lexicographic order
        // of keys equals numeric order of versions, even across digit-count
        // boundaries.
        assert!(manifest_key(ManifestVersion(9)) < manifest_key(ManifestVersion(10)));
        assert!(manifest_key(ManifestVersion(99)) < manifest_key(ManifestVersion(100)));
        assert!(manifest_key(ManifestVersion(0)) < manifest_key(ManifestVersion(u64::MAX)));
    }

    #[test]
    fn parse_round_trips_manifest_key() {
        for v in [0u64, 1, 42, 1000, u64::MAX] {
            let key = manifest_key(ManifestVersion(v));
            assert_eq!(parse_manifest_key(&key), Some(ManifestVersion(v)));
        }
    }

    #[test]
    fn parse_rejects_non_manifest_keys() {
        assert_eq!(parse_manifest_key(LATEST_POINTER_KEY), None);
        assert_eq!(parse_manifest_key("db/manifest/_latest"), None);
        assert_eq!(parse_manifest_key("db/data/abc/shard.ncol"), None);
        assert_eq!(parse_manifest_key("db/manifest/42.json"), None); // not padded
        assert_eq!(
            parse_manifest_key("db/manifest/0000000000000000004x.json"),
            None
        );
        assert_eq!(
            parse_manifest_key("db/manifest/00000000000000000042.txt"),
            None
        );
        assert_eq!(parse_manifest_key(""), None);
    }

    #[test]
    fn version_next_is_monotone() {
        assert_eq!(ManifestVersion::GENESIS, ManifestVersion(0));
        assert_eq!(ManifestVersion(0).next(), ManifestVersion(1));
        assert_eq!(ManifestVersion(41).next(), ManifestVersion(42));
        assert!(ManifestVersion(5) < ManifestVersion(6));
    }

    #[test]
    fn genesis_manifest_is_empty_and_exact() {
        let m = Manifest::genesis("2026-06-13T00:00:00Z");
        assert_eq!(m.manifest_version, ManifestVersion::GENESIS);
        assert_eq!(m.format_version, MANIFEST_FORMAT_VERSION);
        assert!(m.objects.is_empty());
        assert!(m.referenced_keys().is_empty());
        assert_eq!(m.key(), "db/manifest/00000000000000000000.json");
    }

    #[test]
    fn manifest_json_round_trips() {
        let mut m = Manifest::genesis("2026-06-13T00:00:00Z");
        m.manifest_version = ManifestVersion(7);
        m.objects.push(ObjectRef::new(
            "db/data/abc123/nodes-Person-000001.ncol",
            ObjectKind::Ncol,
        ));
        m.objects.push(ObjectRef::new(
            "db/data/def456/adj-FOLLOWS-out-000001.adj",
            ObjectKind::Adj,
        ));
        let bytes = m.to_bytes().expect("serialise");
        let back = Manifest::from_bytes(&bytes).expect("parse");
        assert_eq!(m, back);
    }

    #[test]
    fn referenced_keys_includes_objects_and_stats_blobs() {
        let mut m = Manifest::genesis("t");
        m.objects
            .push(ObjectRef::new("db/data/h1/a.ncol", ObjectKind::Ncol));
        m.objects
            .push(ObjectRef::new("db/data/h2/b.adj", ObjectKind::Adj));
        m.stats_blobs.push(StatsBlobRef::new("db/stats/h3.stats"));
        let keys = m.referenced_keys();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"db/data/h1/a.ncol".to_string()));
        assert!(keys.contains(&"db/data/h2/b.adj".to_string()));
        assert!(keys.contains(&"db/stats/h3.stats".to_string()));
    }

    #[test]
    fn from_bytes_fails_closed_on_newer_format_version() {
        let mut m = Manifest::genesis("t");
        m.format_version = MANIFEST_FORMAT_VERSION + 1;
        let bytes = serde_json::to_vec(&m).unwrap();
        let err = Manifest::from_bytes(&bytes).unwrap_err();
        assert!(matches!(
            err,
            ManifestParseError::UnsupportedFormatVersion { .. }
        ));
    }

    #[test]
    fn from_bytes_rejects_garbage() {
        let err = Manifest::from_bytes(b"not json at all").unwrap_err();
        assert!(matches!(err, ManifestParseError::Json(_)));
    }

    #[test]
    fn unknown_object_kind_is_preserved_on_round_trip() {
        // A reader that does not recognise a kind must still enumerate the
        // object (durability barrier) — it round-trips as Other.
        let mut m = Manifest::genesis("t");
        m.objects.push(ObjectRef::new(
            "db/data/h9/mystery.xyz",
            ObjectKind::Other("xyz".to_string()),
        ));
        let bytes = m.to_bytes().unwrap();
        let back = Manifest::from_bytes(&bytes).unwrap();
        assert_eq!(m, back);
        assert_eq!(back.referenced_keys(), vec!["db/data/h9/mystery.xyz"]);
    }

    #[test]
    fn schema_meta_colocation_defaults_to_true() {
        let mut s = SchemaMeta::default();
        assert!(s.is_colocated("FOLLOWS")); // absent => co-located (K=8)
        s.colocated_projection.insert("WIDE".to_string(), false);
        assert!(!s.is_colocated("WIDE")); // explicit K=9 fallback
        assert!(s.is_colocated("FOLLOWS"));
    }

    /// Round-trip invariant over a generated matrix of manifests
    /// (`testing-and-benchmarks.md` §2: storage-format round-trips). For a wide
    /// variety of structurally-distinct manifests, `bytes → struct → bytes →
    /// struct` is the identity (both the struct *and* its serialised bytes are
    /// stable), proving the encoding is lossless and deterministic. We build the
    /// matrix in-tree (no proptest dependency) so the lockfile / license surface
    /// stays minimal while still covering the property's intent across many
    /// shapes.
    #[test]
    fn manifest_round_trip_is_lossless_and_stable_over_a_matrix() {
        use stats::{DegreeStats, Direction, EstimatorParams, Freshness, StatsBlobRef};

        let kinds = [
            ObjectKind::Ncol,
            ObjectKind::Adj,
            ObjectKind::Idx,
            ObjectKind::Other("future".to_string()),
        ];
        let versions = [0u64, 1, 7, 1_000, u64::MAX];
        let freshnesses = [
            Freshness::Exact,
            Freshness::Estimated,
            Freshness::Stale,
            Freshness::Absent,
        ];

        for (i, &v) in versions.iter().enumerate() {
            let mut m = Manifest::genesis("2026-06-13T18:24:00Z");
            m.manifest_version = ManifestVersion(v);
            m.stats.as_of_version = ManifestVersion(v);
            m.stats.freshness = freshnesses[i % freshnesses.len()];
            m.stats.total_node_count = v.wrapping_mul(3);
            m.stats.estimator_params = EstimatorParams {
                hll_precision: 12 + (i as u8),
                mcv_h: 16,
                hist_q: 48,
                sketch_seed: v,
            };

            // A handful of objects of every kind.
            for (j, kind) in kinds.iter().enumerate() {
                m.objects.push(ObjectRef::new(
                    format!("db/data/hash{i}{j}/shard-{j}.dat"),
                    kind.clone(),
                ));
            }
            // Labels, rel-types, projection flags.
            m.schema.labels = vec!["Person".to_string(), "Account".to_string()];
            m.schema.rel_types = vec!["FOLLOWS".to_string(), "OWNS".to_string()];
            m.schema
                .colocated_projection
                .insert("FOLLOWS".to_string(), i % 2 == 0);
            m.stats.set_label_count("Person", v);
            m.stats.set_degree(
                "FOLLOWS",
                Direction::Out,
                DegreeStats {
                    edge_count: v.wrapping_add(5),
                    p99_deg: (i as u32) * 10,
                    max_deg: (i as u32) * 1_000_000,
                },
            );
            m.stats.set_degree(
                "FOLLOWS",
                Direction::In,
                DegreeStats {
                    edge_count: v.wrapping_add(5),
                    p99_deg: (i as u32) * 7,
                    max_deg: (i as u32) * 2_000_000,
                },
            );
            m.stats_blobs
                .push(StatsBlobRef::new(format!("db/stats/blob{i}.stats")));

            // bytes -> struct -> bytes -> struct identity.
            let bytes1 = m.to_bytes().expect("serialise");
            let back1 = Manifest::from_bytes(&bytes1).expect("parse");
            assert_eq!(m, back1, "struct round-trip identity (v={v})");
            let bytes2 = back1.to_bytes().expect("re-serialise");
            assert_eq!(bytes1, bytes2, "byte round-trip stability (v={v})");
            let back2 = Manifest::from_bytes(&bytes2).expect("re-parse");
            assert_eq!(back1, back2, "second struct round-trip (v={v})");

            // The reference set survives intact (every key recoverable).
            assert_eq!(back1.referenced_keys().len(), kinds.len() + 1);
        }
    }
}
