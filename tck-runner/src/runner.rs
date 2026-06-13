//! Discovering, parsing, and executing the vendored TCK feature corpus.

use std::path::{Path, PathBuf};

use gherkin::{Feature, GherkinEnv, Scenario};

use crate::engine::Engine;
use crate::report::Summary;
use crate::scenario::classify;

/// Recursively collect every `*.feature` file under `root`, sorted for
/// deterministic, reproducible runs.
pub fn discover_features(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect(root, &mut out)?;
    out.sort();
    Ok(out)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "feature") {
            out.push(path);
        }
    }
    Ok(())
}

/// Every scenario in a parsed feature, including those nested inside `Rule:`
/// blocks (the TCK 2024.3 corpus uses none, but newer releases may).
fn all_scenarios(feature: &Feature) -> Vec<&Scenario> {
    let mut scenarios: Vec<&Scenario> = feature.scenarios.iter().collect();
    for rule in &feature.rules {
        scenarios.extend(rule.scenarios.iter());
    }
    scenarios
}

/// Run every scenario in a single feature file through a freshly built engine
/// per scenario, returning the file's [`Summary`].
///
/// `make_engine` is called once per scenario so each starts from a clean
/// engine, mirroring the TCK's per-scenario isolation contract.
pub fn run_feature<E, F>(path: &Path, mut make_engine: F) -> Summary
where
    E: Engine,
    F: FnMut() -> E,
{
    let mut summary = Summary::default();
    match Feature::parse_path(path, GherkinEnv::default()) {
        Ok(feature) => {
            for scenario in all_scenarios(&feature) {
                let mut engine = make_engine();
                summary.record(classify(scenario, &mut engine));
            }
        }
        Err(_) => {
            // A feature file we cannot parse is a corpus/harness problem, not a
            // language miss. Surface it separately so it never masquerades as a
            // pending scenario.
            summary.parse_errors += 1;
        }
    }
    summary
}

/// Run the entire corpus under `root`, building a fresh engine per scenario.
///
/// Returns the merged [`Summary`] across every discovered feature file.
pub fn run_suite<E, F>(root: &Path, mut make_engine: F) -> std::io::Result<Summary>
where
    E: Engine,
    F: FnMut() -> E,
{
    let mut total = Summary::default();
    for path in discover_features(root)? {
        let file_summary = run_feature(&path, &mut make_engine);
        total.merge(&file_summary);
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{ExecOutcome, PendingEngine};
    use std::fs;

    /// A directory holding a couple of synthetic `.feature` files used to prove
    /// the harness counts pass / pending / fail correctly. Created under a
    /// unique temp dir so parallel test runs never collide.
    struct FixtureDir {
        root: PathBuf,
    }

    impl FixtureDir {
        fn new(tag: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "tck-runner-test-{}-{}-{}",
                tag,
                std::process::id(),
                fixture_nonce(),
            ));
            fs::create_dir_all(&root).unwrap();
            FixtureDir { root }
        }
        fn write(&self, name: &str, contents: &str) {
            let path = self.root.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, contents).unwrap();
        }
    }

    impl Drop for FixtureDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn fixture_nonce() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        N.fetch_add(1, Ordering::Relaxed)
    }

    const PASSING_FEATURE: &str = r#"
Feature: Synthetic-Pass
  Scenario: trivially passing scenario
    Given any graph
    When executing query:
      """
      RETURN 1 AS n
      """
    Then the result should be, in any order:
      | n |
      | 1 |
"#;

    const FAILING_FEATURE: &str = r#"
Feature: Synthetic-Fail
  Scenario: trivially failing scenario
    Given any graph
    When executing query:
      """
      RETURN 1 AS n
      """
    Then the result should be, in any order:
      | n |
      | 999 |
"#;

    #[test]
    fn discover_finds_nested_feature_files_sorted() {
        let dir = FixtureDir::new("discover");
        dir.write("b/second.feature", PASSING_FEATURE);
        dir.write("a/first.feature", PASSING_FEATURE);
        dir.write("notes.txt", "not a feature");
        let found = discover_features(&dir.root).unwrap();
        assert_eq!(found.len(), 2);
        assert!(found[0].ends_with("a/first.feature"));
        assert!(found[1].ends_with("b/second.feature"));
    }

    #[test]
    fn stub_engine_counts_everything_pending() {
        let dir = FixtureDir::new("pending");
        dir.write("pass.feature", PASSING_FEATURE);
        dir.write("fail.feature", FAILING_FEATURE);
        let summary = run_suite(&dir.root, || PendingEngine).unwrap();
        assert_eq!(summary.total, 2);
        assert_eq!(summary.pending, 2);
        assert_eq!(summary.pass, 0);
        assert_eq!(summary.fail, 0);
        assert_eq!(summary.parse_errors, 0);
        assert_eq!(summary.pass_rate(), 0.0);
    }

    /// An engine that always answers `RETURN 1 AS n` with the row `1`. The
    /// passing fixture matches it; the failing fixture expects `999` and so
    /// must be counted `fail`.
    struct OnesEngine;
    impl Engine for OnesEngine {
        fn execute(&mut self, query: &str) -> ExecOutcome {
            if query.trim() == "RETURN 1 AS n" {
                ExecOutcome::Rows {
                    columns: vec!["n".into()],
                    rows: vec![vec!["1".into()]],
                }
            } else {
                ExecOutcome::Unsupported
            }
        }
    }

    #[test]
    fn harness_counts_pass_and_fail_correctly() {
        let dir = FixtureDir::new("passfail");
        dir.write("pass.feature", PASSING_FEATURE);
        dir.write("fail.feature", FAILING_FEATURE);
        let summary = run_suite(&dir.root, || OnesEngine).unwrap();
        assert_eq!(summary.total, 2, "two scenarios discovered");
        assert_eq!(summary.pass, 1, "the matching scenario passes");
        assert_eq!(summary.fail, 1, "the mismatching scenario fails");
        assert_eq!(summary.pending, 0);
        assert!((summary.pass_rate() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn unparseable_feature_is_counted_as_parse_error_not_pending() {
        let dir = FixtureDir::new("parse-err");
        dir.write("broken.feature", "this is not valid gherkin at all {{{");
        let summary = run_suite(&dir.root, || PendingEngine).unwrap();
        assert_eq!(summary.parse_errors, 1);
        assert_eq!(summary.total, 0);
        assert_eq!(summary.pending, 0);
    }

    #[test]
    fn missing_root_yields_empty_summary() {
        let summary = run_suite(Path::new("/nonexistent/tck/path"), || PendingEngine).unwrap();
        assert_eq!(summary.total, 0);
        assert_eq!(summary.parse_errors, 0);
    }
}
