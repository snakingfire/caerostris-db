//! CI-wiring checks for the harness scripts (rubric Cat. 12, board item T-0004).
//!
//! These guard that the board/pace dashboard, the epoch hand-off generator, and
//! the STOP-sentinel checkpoint stay reachable from CI — so they cannot silently
//! rot. Cheap file-content assertions, in the spirit of `tests/repo_hygiene.rs`.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read required file {}: {e}", path.display()))
}

#[test]
fn ci_invokes_the_dashboard_generator() {
    let ci = read(".github/workflows/ci.yml");
    assert!(
        ci.contains("scripts/board/dashboard.sh"),
        "CI must expose the dashboard generator as a callable step (T-0004)"
    );
}

#[test]
fn ci_exercises_the_harness_scripts() {
    let ci = read(".github/workflows/ci.yml");
    // The checkpoint + epoch hand-off generators must be smoke-run in CI so a
    // syntax/behaviour regression in them fails the build, not a live recycle.
    assert!(
        ci.contains("scripts/board/checkpoint.sh"),
        "CI must smoke-run the STOP-sentinel checkpoint script (T-0004)"
    );
    assert!(
        ci.contains("scripts/board/epoch-handoff.sh"),
        "CI must smoke-run the epoch hand-off generator (T-0004)"
    );
}

#[test]
fn harness_scripts_are_present_and_executable() {
    for rel in [
        "scripts/board/dashboard.sh",
        "scripts/board/checkpoint.sh",
        "scripts/board/epoch-handoff.sh",
    ] {
        let path = repo_root().join(rel);
        assert!(path.exists(), "{rel} must exist (T-0004)");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert!(mode & 0o111 != 0, "{rel} must be executable (chmod +x)");
        }
    }
}

#[test]
fn epoch_recycling_doc_is_linked_from_process_docs() {
    // The procedure doc must exist and the operating model's self-improvement
    // section should be discoverable; we assert the doc carries its rubric anchor.
    let doc = read("docs/process/epoch-recycling.md");
    assert!(
        doc.contains("Cat. 12") || doc.contains("Cat 12"),
        "epoch-recycling.md must cite its rubric anchor (Cat. 12)"
    );
    assert!(
        doc.contains("EPIC-010"),
        "epoch-recycling.md must reference EPIC-010 (harden the harness)"
    );
}
