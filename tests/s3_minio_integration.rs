//! Integration tests for the S3-backed [`ObjectStore`] (the demo keystone),
//! run against the swarm's local MinIO mock.
//!
//! These prove the *object-storage-native* claim with real objects: the
//! [`S3CliStore`] persists the demo graph to a real S3/MinIO bucket and the
//! executor answers openCypher `MATCH` queries by reading those objects back.
//!
//! ## Provisioning & skipping
//!
//! The shared MinIO mock is provisioned by `scripts/env/up.sh`; an isolated
//! bucket comes from `eval "$(scripts/env/bucket.sh demo)"`. These tests are
//! **environment-gated**: if `CAEROSTRIS_S3_ENDPOINT` / `CAEROSTRIS_S3_BUCKET`
//! are unset, or the endpoint is unreachable, or the `aws` CLI is missing, the
//! test prints a skip notice and passes — so the default `cargo test` suite is
//! green on a machine without the mock, while CI (which runs `up.sh` first)
//! exercises the real path.

use std::process::Command;

use caerostris_db::demo::{build_social_graph, load_graph, persist_graph, run_minio_demo};
use caerostris_db::model::PropertyValue;
use caerostris_db::storage::{ObjectStore, S3CliStore, StoreError};

/// Build a store against the swarm env, or return `None` (with a printed
/// reason) if MinIO is not available — in which case the caller skips.
fn store_or_skip(test_prefix: &str) -> Option<S3CliStore> {
    // The `aws` CLI is mandatory for this backend.
    if Command::new("aws").arg("--version").output().is_err() {
        eprintln!("SKIP: `aws` CLI not found; skipping MinIO integration test.");
        return None;
    }
    let endpoint = match std::env::var("CAEROSTRIS_S3_ENDPOINT") {
        Ok(e) => e,
        Err(_) => {
            eprintln!(
                "SKIP: CAEROSTRIS_S3_ENDPOINT unset; run scripts/env/up.sh \
                 + eval \"$(scripts/env/bucket.sh demo)\" to enable."
            );
            return None;
        }
    };
    let bucket = match std::env::var("CAEROSTRIS_S3_BUCKET") {
        Ok(b) => b,
        Err(_) => {
            eprintln!("SKIP: CAEROSTRIS_S3_BUCKET unset; run scripts/env/bucket.sh demo.");
            return None;
        }
    };
    // Unique per-test prefix so parallel tests within one bucket never collide.
    let prefix = format!(
        "{}it/{}/{}/",
        std::env::var("CAEROSTRIS_S3_PREFIX").unwrap_or_default(),
        test_prefix,
        std::process::id()
    );
    let store = S3CliStore::new(endpoint, bucket, prefix);

    // Reachability probe: ensure the bucket exists; if the endpoint is down the
    // CLI fails and we skip rather than fail.
    match store.ensure_bucket() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("SKIP: could not reach/create bucket ({e}); is MinIO up?");
            return None;
        }
    }
    // A round-trip probe confirms the endpoint actually serves objects.
    let mut probe = store.clone();
    if probe.put("__probe__", b"ok".to_vec()).is_err() {
        eprintln!("SKIP: MinIO put probe failed; endpoint not serving objects.");
        return None;
    }
    let _ = probe.delete("__probe__");
    Some(store)
}

/// Clean every object under the store's prefix so reruns start clean.
fn cleanup(store: &mut S3CliStore) {
    if let Ok(keys) = store.list("") {
        for key in keys {
            let _ = store.delete(&key);
        }
    }
}

#[test]
fn s3_put_get_list_range_round_trip() {
    let Some(mut store) = store_or_skip("crud") else {
        return;
    };
    cleanup(&mut store);

    // put + get.
    store.put("a/one.json", b"hello world".to_vec()).unwrap();
    assert_eq!(store.get("a/one.json").unwrap(), b"hello world");

    // get on a missing key is NotFound.
    assert!(matches!(
        store.get("a/missing").unwrap_err(),
        StoreError::NotFound(_)
    ));

    // list returns the prefixed keys (with our prefix stripped back off).
    store.put("a/two.json", b"second".to_vec()).unwrap();
    store.put("b/three.json", b"third".to_vec()).unwrap();
    let listed = store.list("a/").unwrap();
    assert_eq!(listed, vec!["a/one.json", "a/two.json"]);

    // get_range returns a sub-slice ("hello world"[0..5] == "hello").
    let slice = store.get_range("a/one.json", 0, 5).unwrap();
    assert_eq!(slice, b"hello");
    // Mid-string range ("world").
    let mid = store.get_range("a/one.json", 6, 11).unwrap();
    assert_eq!(mid, b"world");

    // delete removes the object and is idempotent on a missing key.
    store.delete("a/one.json").unwrap();
    assert!(matches!(
        store.get("a/one.json").unwrap_err(),
        StoreError::NotFound(_)
    ));
    store.delete("a/one.json").unwrap(); // idempotent

    cleanup(&mut store);
}

#[test]
fn persist_then_query_round_trip_against_minio() {
    let Some(mut store) = store_or_skip("persist") else {
        return;
    };
    cleanup(&mut store);

    // The bucket prefix starts empty.
    assert!(store.list("").unwrap().is_empty());

    // Persist the social graph as real objects, then reload and verify.
    let graph = build_social_graph();
    let written = persist_graph(&mut store, &graph).unwrap();
    assert_eq!(written.len(), graph.nodes().len() + graph.edges().len());

    // The objects really exist in the bucket now.
    let nodes = store.list("nodes/").unwrap();
    let edges = store.list("edges/").unwrap();
    assert_eq!(
        nodes.len(),
        6,
        "4 people + 2 companies persisted as objects"
    );
    assert_eq!(edges.len(), 7, "3 KNOWS + 4 WORKS_AT persisted as objects");

    // Read the graph back OUT of S3 and confirm a property survived the trip.
    let loaded = load_graph(&store).unwrap();
    assert_eq!(loaded.nodes().len(), 6);
    assert_eq!(loaded.edges().len(), 7);
    let alice = loaded
        .nodes()
        .iter()
        .find(|n| n.property("name") == Some(&PropertyValue::String("Alice".into())))
        .expect("Alice loaded from S3");
    assert_eq!(alice.property("age"), Some(&PropertyValue::Integer(30)));

    cleanup(&mut store);
}

#[test]
fn full_minio_demo_narration_runs_against_minio() {
    let Some(mut store) = store_or_skip("narration") else {
        return;
    };
    cleanup(&mut store);

    let mut buf: Vec<u8> = Vec::new();
    run_minio_demo(&mut buf, &mut store).expect("minio demo runs end to end against MinIO");
    let text = String::from_utf8(buf).unwrap();

    assert!(text.contains("object store is EMPTY"));
    assert!(text.contains("nodes/0.json"));
    assert!(text.contains("Read the graph back from the store"));
    // Q1 returns Alice from the reloaded-from-S3 graph.
    assert!(text.contains("name: 'Alice'"));
    assert!(text.contains("demo complete"));

    cleanup(&mut store);
}
