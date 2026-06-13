//! Integration check (rubric Cat. 12): every dependency resolved in the real
//! `Cargo.lock` must be recorded in `docs/licenses/manifest.toml` with an
//! approved, permissive SPDX identifier.
//!
//! This is the automated guard the board item T-0039 calls for: "a check flags a
//! new dep without a manifest entry." When a new dependency is added to
//! `Cargo.toml`, `cargo` writes it into `Cargo.lock`; if the author forgot to
//! record it in the manifest (or recorded a non-permissive license), this test
//! fails with an actionable message and CI goes red.

use std::path::PathBuf;

use caerostris_db::licenses::{check, parse_lockfile, parse_manifest};

/// Workspace members that are not third-party dependencies.
///
/// Includes the sibling `python/` workspace member (`caerostris-python`,
/// T-0030): that crate has its own `Cargo.lock`, but it is our own code, not a
/// third-party dependency needing a manifest entry.
const OWN_CRATES: &[&str] = &["caerostris-db", "caerostris-python"];

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is the crate root, which is the repo root for this
    // single-crate layout.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Assert that every third-party crate in `lockfile_rel` is recorded in the
/// shared manifest with an approved, permissive SPDX id.
fn assert_lockfile_recorded(lockfile_rel: &str) {
    let root = repo_root();

    let lock_path = root.join(lockfile_rel);
    let manifest_path = root.join("docs/licenses/manifest.toml");

    let lock = std::fs::read_to_string(&lock_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", lock_path.display()));
    let manifest_src = std::fs::read_to_string(&manifest_path).unwrap_or_else(|e| {
        panic!(
            "failed to read the license manifest at {} ({e}). \
             Every dependency must be recorded there — see \
             docs/process/open-source-guardrails.md",
            manifest_path.display()
        )
    });

    let locked = parse_lockfile(&lock, OWN_CRATES);
    let manifest = parse_manifest(&manifest_src);

    let violations = check(&locked, &manifest);

    assert!(
        violations.is_empty(),
        "license-manifest check failed for {lockfile_rel}:\n{}",
        violations
            .iter()
            .map(|v| format!("  - {v}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn lockfile_dependencies_are_all_recorded_and_permissive() {
    assert_lockfile_recorded("Cargo.lock");
}

/// The `python/` bindings crate (T-0030) is its own isolated workspace with its
/// own `Cargo.lock` (so PyO3's tree stays out of the engine lockfile). It is
/// invisible to the root `cargo deny` job and the root lockfile check above, so
/// audit its dependencies against the same shared manifest here. A new crate in
/// the PyO3 tree that is not recorded — or carries a non-permissive license —
/// fails CI just like a root-workspace dependency would.
#[test]
fn python_lockfile_dependencies_are_all_recorded_and_permissive() {
    assert_lockfile_recorded("python/Cargo.lock");
}

/// The manifest must at minimum exist and be parseable so the guard above is
/// never silently skipped.
#[test]
fn manifest_file_exists() {
    let manifest_path = repo_root().join("docs/licenses/manifest.toml");
    assert!(
        manifest_path.exists(),
        "missing license manifest: {} — create it (see docs/licenses/README.md)",
        manifest_path.display()
    );
}
