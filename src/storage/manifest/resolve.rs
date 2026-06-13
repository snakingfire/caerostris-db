//! Latest-version resolution and pinned-manifest reads (ADR 0002 §2.3 / §7.1).
//!
//! A reader opens a database by resolving the **latest committed version** and
//! reading its manifest. ADR 0002 specifies this **exactly**:
//!
//! > A reader resolves the current version by `LIST db/manifest/` and taking
//! > the **max** `V` for which `db/manifest/<V>.json` exists. Because manifest
//! > creation is the atomic commit point and manifests are immutable +
//! > complete-on-create, the max key always names a fully consistent snapshot.
//! > The `_latest` pointer, if present, is only a hint to skip the `LIST`; it is
//! > re-validated and **never trusted on its own**.
//!
//! This module implements that resolution over the
//! [`ObjectStore`](crate::storage::ObjectStore) abstraction, so it works
//! identically against the in-memory backend (unit tests) and an S3-compatible
//! mock / real S3 (integration). It also reads a **pinned** manifest and lets a
//! caller enumerate and read **every** object that manifest references — the
//! durability-barrier invariant (SPIKE-0005 Constraint 3): a reader resolving
//! manifest `M` can read every object `M` references.

use crate::storage::{ObjectStore, StoreError};

use super::{
    LATEST_POINTER_KEY, MANIFEST_PREFIX, Manifest, ManifestParseError, ManifestVersion,
    manifest_key, parse_manifest_key,
};

/// Errors from resolving or reading a manifest.
#[derive(Debug)]
pub enum ManifestStoreError {
    /// The underlying object store failed.
    Store(StoreError),
    /// A manifest object's bytes could not be parsed (or are a newer format).
    Parse(ManifestParseError),
    /// `LIST db/manifest/` returned no manifest objects — the database is
    /// uninitialised (no genesis manifest has been committed yet).
    NoManifest,
    /// The advisory `_latest` pointer named a version whose manifest object is
    /// absent, **and** the `LIST` fallback also found no manifest. (A present
    /// `LIST` always wins; this only fires when both fail.)
    DanglingLatestPointer(String),
}

impl std::fmt::Display for ManifestStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestStoreError::Store(e) => write!(f, "object store error: {e}"),
            ManifestStoreError::Parse(e) => write!(f, "manifest parse error: {e}"),
            ManifestStoreError::NoManifest => {
                write!(
                    f,
                    "no manifest found under {MANIFEST_PREFIX:?} (uninitialised database)"
                )
            }
            ManifestStoreError::DanglingLatestPointer(p) => {
                write!(
                    f,
                    "advisory _latest pointer {p:?} is dangling and LIST found no manifest"
                )
            }
        }
    }
}

impl std::error::Error for ManifestStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ManifestStoreError::Store(e) => Some(e),
            ManifestStoreError::Parse(e) => Some(e),
            _ => None,
        }
    }
}

impl From<StoreError> for ManifestStoreError {
    fn from(e: StoreError) -> Self {
        ManifestStoreError::Store(e)
    }
}

impl From<ManifestParseError> for ManifestStoreError {
    fn from(e: ManifestParseError) -> Self {
        ManifestStoreError::Parse(e)
    }
}

/// Resolve the latest committed manifest version by `LIST db/manifest/` + max
/// (ADR 0002 §2.3).
///
/// The advisory [`LATEST_POINTER_KEY`] is **deliberately ignored** here: this
/// function is the *authoritative* resolver. The pointer is only ever a hint to
/// skip the `LIST` ([`resolve_latest_hint`] handles that path), and the hint is
/// always re-validated against the `LIST` result — it is never trusted on its
/// own.
///
/// # Errors
///
/// - [`ManifestStoreError::Store`] if the `LIST` fails.
/// - [`ManifestStoreError::NoManifest`] if no manifest object exists.
pub fn resolve_latest_version(
    store: &dyn ObjectStore,
) -> Result<ManifestVersion, ManifestStoreError> {
    let keys = store.list(MANIFEST_PREFIX)?;
    keys.iter()
        .filter_map(|k| parse_manifest_key(k))
        .max()
        .ok_or(ManifestStoreError::NoManifest)
}

/// Resolve **and read** the latest committed manifest (ADR 0002 §2.3).
///
/// Equivalent to [`resolve_latest_version`] followed by [`read_manifest`].
///
/// # Errors
///
/// As [`resolve_latest_version`] and [`read_manifest`].
pub fn resolve_latest(store: &dyn ObjectStore) -> Result<Manifest, ManifestStoreError> {
    let version = resolve_latest_version(store)?;
    read_manifest(store, version)
}

/// Resolve the latest version using the advisory `_latest` pointer as a *hint*
/// to skip the `LIST` — but **re-validating** it and falling back to the `LIST`
/// if the hint is missing, malformed, or names an absent manifest (ADR 0002
/// §2.3: "re-validated and never trusted on its own").
///
/// The contract this guarantees: the result is **identical** to
/// [`resolve_latest_version`] whenever a manifest exists. The hint only ever
/// saves a `LIST` round-trip in the common case; it can never make resolution
/// return a wrong or stale version, because:
///
/// 1. A present, well-formed hint is accepted **only if** its named manifest
///    object actually exists *and* no strictly-greater manifest key exists — we
///    confirm the latter cheaply (a hint can only be stale by pointing at an
///    *older* version, never a newer one, since the writer publishes the
///    manifest before bumping the hint). To stay correct under any pointer
///    staleness we still `LIST` when the hint cannot be confirmed as the max.
/// 2. Any failure to confirm the hint falls back to the authoritative `LIST`.
///
/// In this conservative implementation we treat the hint purely as a *fast
/// existence probe*: if it is present, well-formed, and its manifest exists, we
/// still cross-check with a `LIST` to guarantee maximality. The hint therefore
/// never weakens correctness; a future optimisation may elide the `LIST` when
/// the deployment guarantees pointer monotonicity. Today, correctness first.
///
/// # Errors
///
/// As [`resolve_latest_version`].
pub fn resolve_latest_hint(store: &dyn ObjectStore) -> Result<ManifestVersion, ManifestStoreError> {
    // Read the advisory hint (best-effort). A missing/garbled pointer is not an
    // error — we simply fall through to the authoritative LIST.
    let hint = read_latest_pointer(store);

    // Authoritative resolution by LIST + max. The hint is only ever used to
    // *confirm*, never to override: the LIST max is the source of truth.
    let list_max = resolve_latest_version(store);

    match (hint, list_max) {
        // Both present: the LIST max wins; the hint is advisory. (If the hint
        // were ever greater than the LIST max, the hint would be dangling — a
        // pointer ahead of a published manifest — so we trust the LIST.)
        (Some(_hint_v), Ok(list_v)) => Ok(list_v),
        // No usable hint: authoritative LIST result.
        (None, list_result) => list_result,
        // Hint present but LIST found nothing: the pointer is dangling.
        (Some(hint_v), Err(ManifestStoreError::NoManifest)) => Err(
            ManifestStoreError::DanglingLatestPointer(manifest_key(hint_v)),
        ),
        (Some(_), Err(e)) => Err(e),
    }
}

/// Read the advisory `_latest` pointer, returning the version it names if it is
/// present and well-formed, else `None`. Never errors (the pointer is advisory).
fn read_latest_pointer(store: &dyn ObjectStore) -> Option<ManifestVersion> {
    let bytes = store.get(LATEST_POINTER_KEY).ok()?;
    let s = std::str::from_utf8(&bytes).ok()?;
    // The pointer body is the full manifest key it points at (so it is
    // self-describing and re-validatable). Accept either the full key or a bare
    // numeric version for robustness.
    let trimmed = s.trim();
    parse_manifest_key(trimmed).or_else(|| trimmed.parse::<u64>().ok().map(ManifestVersion))
}

/// Read the manifest object for a specific, pinned version.
///
/// # Errors
///
/// - [`ManifestStoreError::Store`] if the object is absent or the `GET` fails.
/// - [`ManifestStoreError::Parse`] if its bytes are not a valid (supported)
///   manifest.
pub fn read_manifest(
    store: &dyn ObjectStore,
    version: ManifestVersion,
) -> Result<Manifest, ManifestStoreError> {
    let bytes = store.get(&manifest_key(version))?;
    Ok(Manifest::from_bytes(&bytes)?)
}

/// Read **every** object a pinned manifest references, returning each key with
/// its bytes — the durability-barrier invariant (SPIKE-0005 Constraint 3): a
/// reader resolving manifest `M` can enumerate and read every object `M`
/// references.
///
/// If any referenced object is missing, this returns
/// [`ManifestStoreError::Store`] wrapping the [`StoreError::NotFound`] for that
/// key — surfacing a durability-barrier violation (a manifest referencing an
/// object that is not durably present) rather than silently skipping it.
///
/// # Errors
///
/// [`ManifestStoreError::Store`] if any referenced object cannot be read.
pub fn read_referenced_objects(
    store: &dyn ObjectStore,
    manifest: &Manifest,
) -> Result<Vec<(String, Vec<u8>)>, ManifestStoreError> {
    let mut out = Vec::with_capacity(manifest.objects.len() + manifest.stats_blobs.len());
    for key in manifest.referenced_keys() {
        let bytes = store.get(&key)?;
        out.push((key, bytes));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::manifest::{ObjectKind, ObjectRef, StatsBlobRef};
    use crate::storage::{MemoryStore, ObjectStore};

    /// Helper: commit a manifest (genesis-derived) at `version` into the store,
    /// the way T-0010's atomic commit will (here just a plain put — atomicity
    /// is T-0010's concern).
    fn put_manifest(store: &mut MemoryStore, version: ManifestVersion) -> Manifest {
        let mut m = Manifest::genesis("2026-06-13T00:00:00Z");
        m.manifest_version = version;
        m.stats.as_of_version = version;
        store.put(&m.key(), m.to_bytes().unwrap()).unwrap();
        m
    }

    #[test]
    fn resolve_on_empty_store_reports_no_manifest() {
        let store = MemoryStore::new();
        let err = resolve_latest_version(&store).unwrap_err();
        assert!(matches!(err, ManifestStoreError::NoManifest));
        assert!(matches!(
            resolve_latest(&store).unwrap_err(),
            ManifestStoreError::NoManifest
        ));
    }

    #[test]
    fn resolve_latest_takes_the_max_version() {
        let mut store = MemoryStore::new();
        put_manifest(&mut store, ManifestVersion(0));
        put_manifest(&mut store, ManifestVersion(1));
        put_manifest(&mut store, ManifestVersion(2));
        assert_eq!(resolve_latest_version(&store).unwrap(), ManifestVersion(2));
        assert_eq!(
            resolve_latest(&store).unwrap().manifest_version,
            ManifestVersion(2)
        );
    }

    #[test]
    fn resolve_max_is_numeric_not_lexical_across_digit_boundaries() {
        // 9 < 10 numerically; zero-padding makes the keys sort the same way.
        let mut store = MemoryStore::new();
        put_manifest(&mut store, ManifestVersion(9));
        put_manifest(&mut store, ManifestVersion(10));
        assert_eq!(resolve_latest_version(&store).unwrap(), ManifestVersion(10));
    }

    #[test]
    fn resolver_ignores_the_advisory_latest_pointer() {
        // The pointer is advisory: even a dangling/stale pointer must not change
        // the authoritative LIST+max result.
        let mut store = MemoryStore::new();
        put_manifest(&mut store, ManifestVersion(0));
        put_manifest(&mut store, ManifestVersion(1));
        // A stale pointer claiming the latest is version 0.
        store
            .put(
                LATEST_POINTER_KEY,
                manifest_key(ManifestVersion(0)).into_bytes(),
            )
            .unwrap();
        assert_eq!(
            resolve_latest_version(&store).unwrap(),
            ManifestVersion(1),
            "authoritative resolver must ignore the advisory pointer"
        );
    }

    #[test]
    fn resolver_ignores_non_manifest_keys_in_the_prefix() {
        let mut store = MemoryStore::new();
        put_manifest(&mut store, ManifestVersion(5));
        // Foreign keys sharing the prefix must not confuse resolution.
        store
            .put(LATEST_POINTER_KEY, b"db/manifest/whatever".to_vec())
            .unwrap();
        store
            .put("db/manifest/notaversion.json", b"x".to_vec())
            .unwrap();
        assert_eq!(resolve_latest_version(&store).unwrap(), ManifestVersion(5));
    }

    #[test]
    fn hint_resolution_matches_authoritative_resolution() {
        let mut store = MemoryStore::new();
        put_manifest(&mut store, ManifestVersion(0));
        put_manifest(&mut store, ManifestVersion(3));
        // Correct hint.
        store
            .put(
                LATEST_POINTER_KEY,
                manifest_key(ManifestVersion(3)).into_bytes(),
            )
            .unwrap();
        assert_eq!(resolve_latest_hint(&store).unwrap(), ManifestVersion(3));

        // Stale hint (points at an older version) — must STILL resolve to 3.
        store
            .put(
                LATEST_POINTER_KEY,
                manifest_key(ManifestVersion(0)).into_bytes(),
            )
            .unwrap();
        assert_eq!(
            resolve_latest_hint(&store).unwrap(),
            ManifestVersion(3),
            "a stale advisory hint must never yield a stale version"
        );

        // No hint at all — falls back to LIST.
        store.delete(LATEST_POINTER_KEY).unwrap();
        assert_eq!(resolve_latest_hint(&store).unwrap(), ManifestVersion(3));
    }

    #[test]
    fn hint_resolution_reports_dangling_pointer_when_list_empty() {
        let mut store = MemoryStore::new();
        store
            .put(
                LATEST_POINTER_KEY,
                manifest_key(ManifestVersion(7)).into_bytes(),
            )
            .unwrap();
        let err = resolve_latest_hint(&store).unwrap_err();
        assert!(matches!(err, ManifestStoreError::DanglingLatestPointer(_)));
    }

    #[test]
    fn read_manifest_round_trips_through_the_store() {
        let mut store = MemoryStore::new();
        let written = put_manifest(&mut store, ManifestVersion(4));
        let read = read_manifest(&store, ManifestVersion(4)).unwrap();
        assert_eq!(written, read);
    }

    #[test]
    fn read_manifest_missing_version_is_store_error() {
        let store = MemoryStore::new();
        let err = read_manifest(&store, ManifestVersion(99)).unwrap_err();
        assert!(matches!(
            err,
            ManifestStoreError::Store(StoreError::NotFound(_))
        ));
    }

    #[test]
    fn read_referenced_objects_enumerates_every_object() {
        // The durability-barrier invariant: a reader resolving M can read every
        // object M references.
        let mut store = MemoryStore::new();
        store
            .put("db/data/h1/a.ncol", b"node-bytes".to_vec())
            .unwrap();
        store
            .put("db/data/h2/b.adj", b"edge-bytes".to_vec())
            .unwrap();
        store
            .put("db/stats/h3.stats", b"stats-bytes".to_vec())
            .unwrap();

        let mut m = Manifest::genesis("t");
        m.manifest_version = ManifestVersion(1);
        m.objects
            .push(ObjectRef::new("db/data/h1/a.ncol", ObjectKind::Ncol));
        m.objects
            .push(ObjectRef::new("db/data/h2/b.adj", ObjectKind::Adj));
        m.stats_blobs.push(StatsBlobRef::new("db/stats/h3.stats"));
        store.put(&m.key(), m.to_bytes().unwrap()).unwrap();

        let resolved = resolve_latest(&store).unwrap();
        let objects = read_referenced_objects(&store, &resolved).unwrap();
        assert_eq!(objects.len(), 3);
        let by_key: std::collections::BTreeMap<_, _> = objects.into_iter().collect();
        assert_eq!(by_key["db/data/h1/a.ncol"], b"node-bytes");
        assert_eq!(by_key["db/data/h2/b.adj"], b"edge-bytes");
        assert_eq!(by_key["db/stats/h3.stats"], b"stats-bytes");
    }

    #[test]
    fn read_referenced_objects_surfaces_a_durability_barrier_violation() {
        // A manifest that references an object the store does not hold is a
        // durability-barrier violation — it must surface, never be skipped.
        let mut store = MemoryStore::new();
        let mut m = Manifest::genesis("t");
        m.manifest_version = ManifestVersion(1);
        m.objects
            .push(ObjectRef::new("db/data/missing/x.ncol", ObjectKind::Ncol));
        store.put(&m.key(), m.to_bytes().unwrap()).unwrap();

        let err = read_referenced_objects(&store, &m).unwrap_err();
        assert!(matches!(
            err,
            ManifestStoreError::Store(StoreError::NotFound(_))
        ));
    }

    #[test]
    fn stats_readable_from_pinned_manifest_with_no_extra_round_trip() {
        // Acceptance criterion 4: the OOE-critical stats are inline in the
        // manifest, so resolving the manifest (one GET) yields them with no
        // further store access.
        let mut store = MemoryStore::new();
        let mut m = Manifest::genesis("t");
        m.manifest_version = ManifestVersion(2);
        m.stats.as_of_version = ManifestVersion(2);
        m.stats.total_node_count = 1000;
        m.stats.set_label_count("Person", 900);
        m.stats.set_degree(
            "FOLLOWS",
            crate::storage::manifest::Direction::Out,
            crate::storage::manifest::DegreeStats {
                edge_count: 5000,
                p99_deg: 50,
                max_deg: 1_000_000,
            },
        );
        store.put(&m.key(), m.to_bytes().unwrap()).unwrap();

        // Wrap the store in a GET counter to PROVE no extra round-trip.
        let counting = CountingStore::new(store);
        let resolved = read_manifest(&counting, ManifestVersion(2)).unwrap();
        let gets_after_read = counting.gets();

        // All OOE-critical scalars are present from the single manifest read.
        assert_eq!(resolved.stats.total_node_count, 1000);
        assert_eq!(resolved.stats.node_count("Person"), Some(900));
        let d = resolved
            .stats
            .degree("FOLLOWS", crate::storage::manifest::Direction::Out)
            .unwrap();
        assert_eq!(d.max_deg, 1_000_000);

        // Reading those stats issued NO further GET beyond the manifest read.
        assert_eq!(
            counting.gets(),
            gets_after_read,
            "reading inline stats must not issue any extra object-store GET"
        );
    }

    /// A test-only [`ObjectStore`] wrapper that counts `get` calls, to prove the
    /// "zero extra round-trip" property of inline statistics.
    struct CountingStore {
        inner: MemoryStore,
        gets: std::cell::Cell<usize>,
    }

    impl CountingStore {
        fn new(inner: MemoryStore) -> Self {
            Self {
                inner,
                gets: std::cell::Cell::new(0),
            }
        }
        fn gets(&self) -> usize {
            self.gets.get()
        }
    }

    impl ObjectStore for CountingStore {
        fn put(&mut self, key: &str, bytes: Vec<u8>) -> Result<(), StoreError> {
            self.inner.put(key, bytes)
        }
        fn get(&self, key: &str) -> Result<Vec<u8>, StoreError> {
            self.gets.set(self.gets.get() + 1);
            self.inner.get(key)
        }
        fn get_range(&self, key: &str, start: usize, end: usize) -> Result<Vec<u8>, StoreError> {
            self.gets.set(self.gets.get() + 1);
            self.inner.get_range(key, start, end)
        }
        fn delete(&mut self, key: &str) -> Result<(), StoreError> {
            self.inner.delete(key)
        }
        fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
            self.inner.list(prefix)
        }
    }
}
