//! Integration test that drives the CI grader-inputs shell harness through
//! `cargo nextest run` / `cargo test`, so the behaviour the rubric-grader
//! depends on (the `GRADER_INPUTS` block + the coverage threshold gate) is
//! guarded by the normal Rust test gate and counted in coverage runs.
//!
//! The substantive assertions live in `scripts/ci/grader-inputs.test.sh`
//! (a dependency-light POSIX harness). This test simply runs that harness and
//! fails the build if it reports any failure, plus directly checks the
//! emitted-block format so a regression surfaces in the Rust test output too.

use std::path::PathBuf;
use std::process::Command;

/// Absolute path to the repository root (the crate manifest dir).
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn grader_inputs_shell_harness_passes() {
    let script = repo_root().join("scripts/ci/grader-inputs.test.sh");
    assert!(
        script.is_file(),
        "grader-inputs test harness missing at {}",
        script.display()
    );

    let output = Command::new("bash")
        .arg(&script)
        .output()
        .expect("failed to spawn bash for grader-inputs.test.sh");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "grader-inputs.test.sh failed.\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
}

#[test]
fn grader_inputs_emits_required_block() {
    let script = repo_root().join("scripts/ci/grader-inputs.sh");
    let output = Command::new("bash")
        .arg(&script)
        .args(["--coverage", "0", "--threshold", "0"])
        .args(["--test-pass", "3", "--test-total", "3"])
        .output()
        .expect("failed to spawn bash for grader-inputs.sh");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "gate should pass at threshold 0");
    assert!(
        stdout.contains("GRADER_INPUTS:"),
        "missing header: {stdout}"
    );
    assert!(
        stdout.contains("coverage_pct: 0"),
        "missing coverage_pct: {stdout}"
    );
    assert!(
        stdout.contains("test_pass: 3/3"),
        "missing test_pass: {stdout}"
    );
    assert!(
        stdout.contains("tck_pass_rate: 0/0"),
        "missing tck_pass_rate: {stdout}"
    );
}

#[test]
fn grader_inputs_gate_fails_below_threshold() {
    let script = repo_root().join("scripts/ci/grader-inputs.sh");
    let output = Command::new("bash")
        .arg(&script)
        .args(["--coverage", "10", "--threshold", "90"])
        .output()
        .expect("failed to spawn bash for grader-inputs.sh");

    assert!(
        !output.status.success(),
        "gate must fail when coverage (10) < threshold (90)"
    );
    // The block must still be emitted before the gate fails, so the grader can
    // record the (failing) number rather than seeing nothing.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("GRADER_INPUTS:"),
        "block should be emitted even on gate failure: {stdout}"
    );
}
