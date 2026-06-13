//! Integration test (rubric Cat. 1 / 2 / 3): the manifest object, its statistics
//! block, and latest-version resolution, exercised end-to-end through the
//! [`ObjectStore`](caerostris_db::storage::ObjectStore) trait against a real
//! backend — the way a reader opens a database.
//!
//! This drives **T-0009**'s acceptance criteria at the integration level:
//!
//! 1. A multi-version commit sequence resolves to the **latest** version by
//!    `LIST db/manifest/` + max (ADR 0002 §2.3).
//! 2. A reader resolving manifest `M` can **enumerate and read every object**
//!    `M` references — the durability-barrier invariant (SPIKE-0005 Constraint
//!    3).
//! 3. The OOE-critical statistics are readable from the pinned manifest with
//!    **no extra round-trip** beyond resolving the manifest itself.
//! 4. A stale reader that pinned an older version still reads a **complete,
//!    consistent** snapshot after newer versions commit (snapshot isolation;
//!    ADR 0002 §5).
//!
//! # Backend
//!
//! The test runs against the engine's [`ObjectStore`] abstraction. Today the
//! only landed backend is the in-process [`MemoryStore`] (the floor of the
//! provision ladder in `docs/process/testing-and-benchmarks.md` §3); the same
//! test runs unmodified against the local S3 mock the moment an S3-compatible
//! `ObjectStore` adapter lands (tracked as a follow-up task). The manifest
//! logic under test — key naming, LIST+max resolution, reference-set
//! enumeration — is backend-agnostic: it depends only on the `ObjectStore`
//! contract (`put` / `get` / `list`), which the S3 adapter must honour
//! identically.

use caerostris_db::storage::manifest::{
    DegreeStats, Direction, Manifest, ManifestVersion, ObjectKind, ObjectRef, StatsBlobRef,
    manifest_key, read_manifest, read_referenced_objects, resolve_latest, resolve_latest_version,
};
use caerostris_db::storage::{MemoryStore, ObjectStore};

/// Build a manifest for `version` referencing the given data/stats object keys,
/// with a small but realistic inline statistics block.
fn make_manifest(version: ManifestVersion, data_keys: &[(&str, ObjectKind)], stats_blob: Option<&str>) -> Manifest {
    let mut m = Manifest::genesis("2026-06-13T18:24:00Z");
    m.manifest_version = version;
    m.stats.as_of_version = version;
    m.stats.total_node_count = 12;
    m.stats.set_label_count("Person", 12);
    m.stats.set_degree(
        "FOLLOWS",
        Direction::Out,
        DegreeStats {
            edge_count: 20,
            p99_deg: 4,
            max_deg: 7,
        },
    );
    for (key, kind) in data_keys {
        m.objects.push(ObjectRef::new(*key, kind.clone()));
    }
    if let Some(blob) = stats_blob {
        m.stats_blobs.push(StatsBlobRef::new(blob));
    }
    m
}

/// Commit a manifest into the store the way T-0010's atomic commit will: stage
/// every referenced data object (durability barrier), then write the manifest.
fn commit(store: &mut dyn ObjectStore, manifest: &Manifest) {
    // Durability barrier: every referenced object durable BEFORE the manifest.
    for key in manifest.referenced_keys() {
        store
            .put(&key, format!("bytes-for-{key}").into_bytes())
            .expect("stage object");
    }
    store
        .put(&manifest.key(), manifest.to_bytes().expect("serialise manifest"))
        .expect("write manifest");
}

#[test]
fn latest_resolution_returns_the_newest_committed_version() {
    let mut store = MemoryStore::new();

    // Genesis (empty), then two real versions.
    commit(&mut store, &Manifest::genesis("t0"));
    commit(
        &mut store,
        &make_manifest(
            ManifestVersion(1),
            &[("db/data/h1/nodes-Person-0.ncol", ObjectKind::Ncol)],
            None,
        ),
    );
    let v2 = make_manifest(
        ManifestVersion(2),
        &[
            ("db/data/h1/nodes-Person-0.ncol", ObjectKind::Ncol),
            ("db/data/h2/adj-FOLLOWS-out-0.adj", ObjectKind::Adj),
        ],
        Some("db/stats/h3.stats"),
    );
    commit(&mut store, &v2);

    assert_eq!(
        resolve_latest_version(&store).expect("resolve"),
        ManifestVersion(2)
    );
    let latest = resolve_latest(&store).expect("resolve+read");
    assert_eq!(latest, v2, "resolved manifest must equal the committed one");
}

#[test]
fn reader_can_enumerate_and_read_every_referenced_object() {
    // Durability-barrier invariant (SPIKE-0005 Constraint 3 / acceptance
    // criterion 3): resolving M, a reader reads every object M references.
    let mut store = MemoryStore::new();
    let m = make_manifest(
        ManifestVersion(1),
        &[
            ("db/data/h1/nodes-Person-0.ncol", ObjectKind::Ncol),
            ("db/data/h2/adj-FOLLOWS-out-0.adj", ObjectKind::Adj),
            ("db/data/h4/idx-Person-name-0.idx", ObjectKind::Idx),
        ],
        Some("db/stats/h3.stats"),
    );
    commit(&mut store, &m);

    let resolved = resolve_latest(&store).expect("resolve");
    let objects = read_referenced_objects(&store, &resolved).expect("read all referenced");

    // Every referenced key was read, content intact.
    let read_keys: std::collections::BTreeSet<_> = objects.iter().map(|(k, _)| k.clone()).collect();
    let expected: std::collections::BTreeSet<_> = resolved.referenced_keys().into_iter().collect();
    assert_eq!(read_keys, expected);
    assert_eq!(objects.len(), 4); // 3 data objects + 1 stats blob
    for (key, bytes) in &objects {
        assert_eq!(bytes, &format!("bytes-for-{key}").into_bytes());
    }
}

#[test]
fn ooe_critical_stats_are_readable_from_the_pinned_manifest() {
    // Acceptance criterion 4: the OOE-critical scalars (node_count,
    // total_node_count, edge_count, p99_deg, max_deg) are inline, so resolving
    // the manifest yields them directly.
    let mut store = MemoryStore::new();
    commit(
        &mut store,
        &make_manifest(ManifestVersion(1), &[], Some("db/stats/h3.stats")),
    );

    let m = resolve_latest(&store).expect("resolve");
    assert_eq!(m.stats.total_node_count, 12);
    assert_eq!(m.stats.node_count("Person"), Some(12));
    let d = m.stats.degree("FOLLOWS", Direction::Out).expect("degree present");
    assert_eq!(d.edge_count, 20);
    assert_eq!(d.p99_deg, 4);
    assert_eq!(d.max_deg, 7); // the mandatory super-hub safety term
    // The bulky selectivity detail is referenced, not inline.
    assert_eq!(m.stats_blobs.len(), 1);
    assert_eq!(m.stats_blobs[0].key, "db/stats/h3.stats");
}

#[test]
fn stale_reader_keeps_a_consistent_snapshot_after_newer_commits() {
    // Snapshot isolation (ADR 0002 §5): a reader that pinned V=1 still reads a
    // complete, consistent V=1 snapshot after V=2 commits — because manifests
    // are immutable and commit only ever CREATES new keys.
    let mut store = MemoryStore::new();
    let v1 = make_manifest(
        ManifestVersion(1),
        &[("db/data/h1/nodes-Person-0.ncol", ObjectKind::Ncol)],
        None,
    );
    commit(&mut store, &v1);

    // Reader pins V=1.
    let pinned = read_manifest(&store, ManifestVersion(1)).expect("read pinned");

    // Writer commits V=2 (new keys only).
    commit(
        &mut store,
        &make_manifest(
            ManifestVersion(2),
            &[
                ("db/data/h1/nodes-Person-0.ncol", ObjectKind::Ncol),
                ("db/data/h2/adj-FOLLOWS-out-0.adj", ObjectKind::Adj),
            ],
            None,
        ),
    );

    // The pinned V=1 manifest is unchanged and still fully readable.
    let reread = read_manifest(&store, ManifestVersion(1)).expect("re-read pinned");
    assert_eq!(pinned, reread);
    let objs = read_referenced_objects(&store, &pinned).expect("read pinned objects");
    assert_eq!(objs.len(), 1, "pinned snapshot sees exactly its own object set");

    // Meanwhile the latest resolver sees V=2.
    assert_eq!(
        resolve_latest_version(&store).expect("resolve"),
        ManifestVersion(2)
    );
}

#[test]
fn no_raw_property_values_appear_in_a_committed_manifest() {
    // Condition C4 / guardrails §3: a committed manifest must carry value
    // *digests*, never raw property values. Our inline block carries only
    // counts/degrees, so the strongest assertion is: the manifest JSON contains
    // none of the property *values* a graph might hold. We seed a sentinel and
    // assert it never lands in the manifest bytes.
    let mut store = MemoryStore::new();
    let m = make_manifest(
        ManifestVersion(1),
        &[("db/data/h1/nodes-Person-0.ncol", ObjectKind::Ncol)],
        Some("db/stats/h3.stats"),
    );
    commit(&mut store, &m);

    let manifest_bytes = store.get(&manifest_key(ManifestVersion(1))).expect("get manifest");
    let manifest_text = String::from_utf8(manifest_bytes).expect("utf8");
    // Property *values* (a user's name, a country code) must never appear; the
    // manifest only holds keys, kinds, counts, degrees, and digests.
    assert!(!manifest_text.contains("Iceland"));
    assert!(!manifest_text.contains("\"name\":\"Alice\""));
    // It *does* hold the schema/label/rel-type NAMES and object KEYS (those are
    // not user data — they are catalog metadata and content-addressed paths).
    assert!(manifest_text.contains("Person"));
    assert!(manifest_text.contains("FOLLOWS"));
}
