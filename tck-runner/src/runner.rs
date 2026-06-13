//! Discovering, parsing, and executing the vendored TCK feature corpus.

use std::path::{Path, PathBuf};

use gherkin::{Feature, GherkinEnv, Scenario};

use crate::engine::Engine;
use crate::outline::expand_scenario;
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

/// Every **executable** scenario in a parsed feature — every plain `Scenario:`
/// plus each `Scenario Outline:` expanded into one concrete scenario per
/// `Examples` data row (BUG-0009 / Decision 0013), including those nested inside
/// `Rule:` blocks (the TCK 2024.3 corpus uses none, but newer releases may).
///
/// Expansion is what makes `Summary::total` reflect the conventional openCypher
/// test-case count rather than counting each outline once: a 276-outline /
/// 2541-Examples-row corpus expands from 1615 definitions to ~3880 cases. Each
/// returned scenario has its `<placeholder>` tokens substituted, so the engine
/// never sees literal `<comp>` / `<boolop>` text (which would be a false `fail`
/// or a stuck `pending`).
fn all_scenarios(feature: &Feature) -> Vec<Scenario> {
    let mut scenarios: Vec<Scenario> = Vec::new();
    for scenario in &feature.scenarios {
        scenarios.extend(expand_scenario(scenario));
    }
    for rule in &feature.rules {
        for scenario in &rule.scenarios {
            scenarios.extend(expand_scenario(scenario));
        }
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
                summary.record(classify(&scenario, &mut engine));
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
                ExecOutcome::rows(vec!["n".into()], vec![vec!["1".into()]])
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

    /// A `.feature` whose only assertion is `Then the side effects should be:`.
    /// Drives the full harness path for BUG-0006 / Decision 0012: the side
    /// effects are asserted against the engine's reported `QueryStatistics`.
    const SIDE_EFFECT_FEATURE: &str = r#"
Feature: Synthetic-SideEffects
  Scenario: create then delete reports side effects
    Given an empty graph
    When executing query:
      """
      CREATE (n) DELETE n
      """
    Then the side effects should be:
      | +nodes | 1 |
      | -nodes | 1 |
"#;

    /// An engine that reports `CREATE (n) DELETE n` as creating then deleting
    /// one node — the exact side effects the fixture expects.
    struct CreateDeleteEngine;
    impl Engine for CreateDeleteEngine {
        fn execute(&mut self, query: &str) -> ExecOutcome {
            if query.trim() == "CREATE (n) DELETE n" {
                let mut se = caerostris_db::query::QueryStatistics::new();
                se.record_nodes_created(1);
                se.record_nodes_deleted(1);
                ExecOutcome::rows_with_side_effects(Vec::new(), Vec::new(), se)
            } else {
                ExecOutcome::Unsupported
            }
        }
    }

    #[test]
    fn side_effect_scenario_passes_through_full_harness() {
        let dir = FixtureDir::new("side-effects");
        dir.write("side_effects.feature", SIDE_EFFECT_FEATURE);
        let summary = run_suite(&dir.root, || CreateDeleteEngine).unwrap();
        assert_eq!(summary.total, 1);
        assert_eq!(summary.pass, 1, "matching side effects are a real pass");
        assert_eq!(summary.fail, 0);
        assert_eq!(
            summary.pending, 0,
            "side-effect scenarios are never auto-pending"
        );
    }

    #[test]
    fn side_effect_scenario_is_pending_under_stub_engine() {
        let dir = FixtureDir::new("side-effects-pending");
        dir.write("side_effects.feature", SIDE_EFFECT_FEATURE);
        let summary = run_suite(&dir.root, || PendingEngine).unwrap();
        assert_eq!(summary.total, 1);
        assert_eq!(summary.pending, 1, "unsupported -> pending, never fail");
        assert_eq!(summary.fail, 0);
    }

    // --- Scenario Outline expansion (BUG-0009 / Decision 0013) --------------

    /// A `Scenario Outline:` with three `Examples` data rows plus one plain
    /// `Scenario:`. The conventional openCypher test-case count is 1 + 3 = 4,
    /// not the 2 definitions the unexpanded harness used to report.
    const OUTLINE_FEATURE: &str = r#"
Feature: Synthetic-Outline
  Scenario: a plain one
    Given any graph
    When executing query:
      """
      RETURN 1 AS n
      """
    Then the result should be, in any order:
      | n |
      | 1 |

  Scenario Outline: return <value>
    Given any graph
    When executing query:
      """
      RETURN <value> AS n
      """
    Then the result should be, in any order:
      | n       |
      | <value> |
    Examples:
      | value |
      | 1     |
      | 2     |
      | 3     |
"#;

    #[test]
    fn outline_is_expanded_into_one_scenario_per_examples_row() {
        // The defining file has 2 definitions (1 plain + 1 outline) but 4
        // conventional test cases (1 + 3 Examples rows). `total` must reflect
        // the expanded count — this is the BUG-0009 fix.
        let dir = FixtureDir::new("outline-total");
        dir.write("outline.feature", OUTLINE_FEATURE);
        let summary = run_suite(&dir.root, || PendingEngine).unwrap();
        assert_eq!(
            summary.total, 4,
            "1 plain + 3 expanded outline rows = 4 test cases, not 2 definitions"
        );
        assert_eq!(summary.pending, 4, "stub engine -> every case pending");
        assert_eq!(summary.fail, 0);
    }

    /// An engine that only answers the *substituted* queries (`RETURN 1 AS n`,
    /// `RETURN 2 AS n`, `RETURN 3 AS n`). A literal `RETURN <value> AS n` is
    /// `Unsupported`, so if expansion failed to substitute the placeholder the
    /// scenario would be `pending` — proving the engine never sees `<value>`.
    struct SubstitutedRetEngine;
    impl Engine for SubstitutedRetEngine {
        fn execute(&mut self, query: &str) -> ExecOutcome {
            let q = query.trim();
            match q {
                "RETURN 1 AS n" => ExecOutcome::rows(vec!["n".into()], vec![vec!["1".into()]]),
                "RETURN 2 AS n" => ExecOutcome::rows(vec!["n".into()], vec![vec!["2".into()]]),
                "RETURN 3 AS n" => ExecOutcome::rows(vec!["n".into()], vec![vec!["3".into()]]),
                _ => ExecOutcome::Unsupported,
            }
        }
    }

    #[test]
    fn expanded_scenarios_run_substituted_queries_and_pass() {
        // Each expanded variant carries its substituted query *and* its
        // substituted expected-result cell (`<value>` -> 1/2/3), so all four
        // pass. A surviving `<value>` in either the query or the result cell
        // would force a pending (query) or a fail (result mismatch).
        let dir = FixtureDir::new("outline-pass");
        dir.write("outline.feature", OUTLINE_FEATURE);
        let summary = run_suite(&dir.root, || SubstitutedRetEngine).unwrap();
        assert_eq!(summary.total, 4);
        assert_eq!(
            summary.pass, 4,
            "every substituted variant passes; no literal <value> reached the engine"
        );
        assert_eq!(
            summary.pending, 0,
            "no placeholder survived -> nothing pending"
        );
        assert_eq!(summary.fail, 0, "substituted result cells match -> no fail");
    }
}
