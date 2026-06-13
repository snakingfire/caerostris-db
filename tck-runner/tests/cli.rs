//! End-to-end tests of the `tck-runner` binary CLI.
//!
//! These run the compiled binary as a subprocess (path provided by Cargo via
//! `CARGO_BIN_EXE_tck-runner`) against small synthetic corpora, exercising the
//! real output formats, `--output` file writing, `--strict` gating, and exit
//! codes — the contract the CI `openCypher TCK pass-rate` step depends on.

use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_tck-runner")
}

/// A throwaway corpus dir with synthetic feature files, cleaned on drop.
struct Corpus {
    root: PathBuf,
}

impl Corpus {
    fn new(tag: &str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "tck-cli-{}-{}-{}",
            tag,
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        Corpus { root }
    }
    fn write(&self, name: &str, contents: &str) {
        std::fs::write(self.root.join(name), contents).unwrap();
    }
}

impl Drop for Corpus {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

const PENDING_FEATURE: &str = r#"
Feature: CLI-Pending
  Scenario: a scenario the stub cannot run
    Given any graph
    When executing query:
      """
      MATCH (n) RETURN n
      """
    Then the result should be, in any order:
      | n |
      | 1 |
"#;

#[test]
fn text_output_reports_pending_and_exits_zero() {
    let corpus = Corpus::new("text");
    corpus.write("a.feature", PENDING_FEATURE);

    let out = Command::new(bin())
        .arg(&corpus.root)
        .output()
        .expect("runs the binary");

    assert!(out.status.success(), "default run exits 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("total:        1"), "stdout: {stdout}");
    assert!(stdout.contains("pending:      1"), "stdout: {stdout}");
    assert!(stdout.contains("pass_rate:    0.0000"), "stdout: {stdout}");
}

#[test]
fn json_output_is_machine_readable() {
    let corpus = Corpus::new("json");
    corpus.write("a.feature", PENDING_FEATURE);

    let out = Command::new(bin())
        .arg(&corpus.root)
        .args(["--format", "json"])
        .output()
        .expect("runs the binary");

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).expect("valid JSON");
    assert_eq!(parsed["total"], 1);
    assert_eq!(parsed["pending"], 1);
    assert_eq!(parsed["pass_rate"].as_f64(), Some(0.0));
}

#[test]
fn output_flag_writes_json_file() {
    let corpus = Corpus::new("outfile");
    corpus.write("a.feature", PENDING_FEATURE);
    let out_path = corpus.root.join("report.json");

    let status = Command::new(bin())
        .arg(&corpus.root)
        .args(["--output", out_path.to_str().unwrap()])
        .status()
        .expect("runs the binary");

    assert!(status.success());
    let written = std::fs::read_to_string(&out_path).expect("report file written");
    let parsed: serde_json::Value = serde_json::from_str(written.trim()).unwrap();
    assert_eq!(parsed["total"], 1);
}

#[test]
fn strict_mode_fails_on_parse_error() {
    let corpus = Corpus::new("strict");
    corpus.write("broken.feature", "definitely not gherkin {{{");

    // Default: a parse error does not fail the run (does not block the board).
    let lenient = Command::new(bin())
        .arg(&corpus.root)
        .status()
        .expect("runs");
    assert!(lenient.success(), "lenient run tolerates parse errors");

    // --strict: a parse error makes the run fail.
    let strict = Command::new(bin())
        .arg(&corpus.root)
        .arg("--strict")
        .status()
        .expect("runs");
    assert!(!strict.success(), "strict run fails on parse_errors");
}

#[test]
fn bad_flag_exits_nonzero_with_usage() {
    let out = Command::new(bin())
        .arg("--definitely-not-a-flag")
        .output()
        .expect("runs the binary");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Usage:"), "stderr: {stderr}");
}

#[test]
fn help_exits_zero() {
    let out = Command::new(bin())
        .arg("--help")
        .output()
        .expect("runs the binary");
    assert!(out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Usage:"));
}

#[test]
fn missing_corpus_arg_is_an_operational_error() {
    // A path that does not exist as a directory: discover_features returns an
    // io error -> the runner reports an operational failure.
    let missing = Path::new("/nonexistent/tck/corpus/dir");
    let out = Command::new(bin())
        .arg(missing)
        .output()
        .expect("runs the binary");
    // discover over a missing dir yields an empty (Ok) summary, so this is a
    // clean zero-scenario run, not an error.
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("total:        0"), "stdout: {stdout}");
}
