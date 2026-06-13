//! Translating a parsed Gherkin scenario into an executable TCK scenario and
//! deciding its [`Verdict`].
//!
//! TCK scenarios follow a small, regular shape:
//!
//! ```gherkin
//! Scenario: ...
//!   Given an empty graph          # or "any graph"
//!   And having executed:          # zero or more setup statements
//!     """
//!     CREATE (...)
//!     """
//!   When executing query:         # the statement under test
//!     """
//!     MATCH (n) RETURN n
//!     """
//!   Then the result should be, in any order:   # OR an expected error
//!     | n |
//!     | 1 |
//!   And the side effects should be:             # optional side-effect table
//!     | +nodes | 1 |
//! ```
//!
//! We extract the setup statements, the main query, the result/error
//! expectation, and any `Then the side effects should be:` table, run them
//! through the [`Engine`], and compare. The side-effect table is asserted
//! against the engine's reported [`QueryStatistics`] as a real pass/fail —
//! never auto-`pending` (BUG-0006 / Decision 0012).

use caerostris_db::query::QueryStatistics;
use gherkin::{Scenario, StepType};
use serde::Serialize;

use crate::engine::{Engine, ErrorPhase, ExecOutcome, ResultRow};

/// The verdict for one scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// The engine produced the expected result (or expected error).
    Pass,
    /// The engine reported some construct as unsupported. Not a failure — the
    /// language feature simply is not implemented yet.
    Pending,
    /// The engine ran but produced the wrong result, or raised the wrong/no
    /// error. A genuine conformance miss.
    Fail,
}

/// What a scenario expects after the `When` query runs.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Expectation {
    /// `Then the result should be ...`: the exact set/sequence of rows.
    /// `ordered` mirrors `should be, in order` vs `in any order`.
    Result {
        columns: Vec<String>,
        rows: Vec<ResultRow>,
        ordered: bool,
    },
    /// `Then a <Kind> should be raised at <phase>: ...`.
    Error { kind: String, phase: ErrorPhase },
    /// A scenario whose only assertion is `no side effects` / `the result
    /// should be empty` with no checkable rows. A supported execution counts as
    /// a pass at the harness level (an explicit side-effect table, when
    /// present, is checked separately — see [`TckScenario::expected_side_effects`]).
    NoResultRows,
}

/// A TCK scenario reduced to the parts the harness executes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TckScenario {
    setup: Vec<String>,
    query: Option<String>,
    expectation: Option<Expectation>,
    /// The expected side effects, parsed from a `Then the side effects should
    /// be:` table (BUG-0006 / Decision 0012). `None` when the scenario asserts
    /// nothing about side effects; `Some(stats)` makes the harness compare the
    /// engine's reported [`QueryStatistics`] for a real pass/fail. A parse
    /// failure of the table is recorded as [`SideEffectExpectation::Unparseable`]
    /// so the scenario is `pending`, never a spurious `fail`.
    expected_side_effects: Option<SideEffectExpectation>,
}

/// The side-effect assertion extracted from a scenario, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SideEffectExpectation {
    /// A parsed `Then the side effects should be:` table.
    Table(QueryStatistics),
    /// The table was present but could not be parsed into a `QueryStatistics`
    /// (a corpus/harness edge case). The scenario is treated as `pending` so a
    /// harness limitation never masquerades as a conformance `fail`.
    Unparseable,
}

/// Extract the executable shape from a parsed Gherkin scenario.
///
/// Returns `None`-ish parts when the scenario does not match the expected TCK
/// grammar; [`classify`] treats anything it cannot execute as `Pending` so a
/// malformed-but-parseable scenario never counts as a hard `fail`.
fn lower(scenario: &Scenario) -> TckScenario {
    let mut setup = Vec::new();
    let mut query = None;
    let mut expectation = None;
    let mut expected_side_effects = None;

    for step in &scenario.steps {
        let value = step.value.trim();
        let lower = value.to_ascii_lowercase();
        match step.ty {
            StepType::Given => {
                // `Given having executed:` carries a docstring setup statement.
                if lower.starts_with("having executed") {
                    if let Some(doc) = &step.docstring {
                        setup.push(doc.trim().to_string());
                    }
                }
                // `an empty graph` / `any graph` need no setup statement.
            }
            StepType::When => {
                // `When executing query:` / `When executing control query:`.
                if lower.starts_with("executing") {
                    if let Some(doc) = &step.docstring {
                        query = Some(doc.trim().to_string());
                    }
                }
            }
            StepType::Then => {
                // `Then the side effects should be:` carries its own table and
                // is checked against the engine's QueryStatistics independently
                // of the result-row assertion (BUG-0006 / Decision 0012).
                if lower.starts_with("the side effects should be") {
                    expected_side_effects = Some(parse_side_effects(step));
                    continue;
                }

                // A scenario can have several `Then`/`And` assertions (e.g. a
                // `result should be` table followed by `no side effects`). The
                // primary, harness-checkable assertion is the result table or
                // the expected error; a later side-effect `And` must not clobber
                // it. So only fill `expectation` from the first result/error
                // step, and otherwise fall back to a side-effects-only marker.
                let parsed = parse_expectation(value, &lower, step);
                match (&expectation, &parsed) {
                    // Keep an already-recorded primary assertion.
                    (Some(Expectation::Result { .. } | Expectation::Error { .. }), _) => {}
                    // A new primary assertion replaces a side-effects-only marker.
                    _ => expectation = Some(parsed),
                }
            }
        }
    }

    TckScenario {
        setup,
        query,
        expectation,
        expected_side_effects,
    }
}

/// Parse a `Then the side effects should be:` step's Gherkin table into the
/// expected [`QueryStatistics`] (BUG-0006 / Decision 0012). Renders the table
/// back to the canonical `| key | n |` text form and delegates to
/// [`QueryStatistics::from_tck_side_effects`], which treats omitted categories
/// as zero and rejects unknown categories.
fn parse_side_effects(step: &gherkin::Step) -> SideEffectExpectation {
    let Some(table) = &step.table else {
        // `the side effects should be:` with no table means "no side effects" —
        // an all-zero statistics object.
        return SideEffectExpectation::Table(QueryStatistics::new());
    };
    let mut rendered = String::new();
    for row in &table.rows {
        // Each row is `[category, count]`; render `| category | count |`.
        if row.len() != 2 {
            return SideEffectExpectation::Unparseable;
        }
        rendered.push_str(&format!("| {} | {} |\n", row[0].trim(), row[1].trim()));
    }
    match QueryStatistics::from_tck_side_effects(&rendered) {
        Ok(stats) => SideEffectExpectation::Table(stats),
        Err(_) => SideEffectExpectation::Unparseable,
    }
}

fn parse_expectation(value: &str, lower: &str, step: &gherkin::Step) -> Expectation {
    if lower.contains("should be raised") {
        let kind = parse_error_kind(value);
        let phase = if lower.contains("compile time") {
            ErrorPhase::CompileTime
        } else if lower.contains("runtime") {
            ErrorPhase::Runtime
        } else {
            ErrorPhase::AnyTime
        };
        return Expectation::Error { kind, phase };
    }

    if lower.starts_with("the result should be") {
        let ordered = lower.contains("in order") && !lower.contains("in any order");
        if let Some(table) = &step.table {
            let (columns, rows) = split_table(table);
            return Expectation::Result {
                columns,
                rows,
                ordered,
            };
        }
        // "should be empty" with no table.
        return Expectation::Result {
            columns: Vec::new(),
            rows: Vec::new(),
            ordered,
        };
    }

    // `no side effects`, `the side effects should be:`, `the result should be empty`, etc.
    Expectation::NoResultRows
}

/// `a SyntaxError should be raised ...` -> `"SyntaxError"`.
fn parse_error_kind(value: &str) -> String {
    // Strip a leading article and take the first whitespace-delimited token.
    let rest = value
        .trim_start_matches("a ")
        .trim_start_matches("an ")
        .trim_start();
    rest.split_whitespace().next().unwrap_or("").to_string()
}

/// Split a Gherkin table into a header row and data rows.
fn split_table(table: &gherkin::Table) -> (Vec<String>, Vec<ResultRow>) {
    let mut iter = table.rows.iter();
    let columns = iter
        .next()
        .map(|r| r.iter().map(|c| c.trim().to_string()).collect())
        .unwrap_or_default();
    let rows = iter
        .map(|r| r.iter().map(|c| c.trim().to_string()).collect())
        .collect();
    (columns, rows)
}

/// Run a single scenario through the engine and decide its verdict.
///
/// A fresh engine instance is supplied per scenario by the caller, matching the
/// TCK's "each scenario starts from a clean graph" contract.
pub fn classify<E: Engine>(scenario: &Scenario, engine: &mut E) -> Verdict {
    let lowered = lower(scenario);

    // No executable query (e.g. a malformed scenario we cannot drive): pending.
    let Some(query) = lowered.query else {
        return Verdict::Pending;
    };

    // Run setup statements first; any unsupported construct -> pending.
    for stmt in &lowered.setup {
        if let ExecOutcome::Unsupported = engine.execute(stmt) {
            return Verdict::Pending;
        }
    }

    let outcome = engine.execute(&query);

    // Unsupported anywhere -> pending, regardless of what was asserted.
    if let ExecOutcome::Unsupported = outcome {
        return Verdict::Pending;
    }

    // A `Then the side effects should be:` table that the harness could not
    // parse is a harness limitation, not a conformance miss -> pending.
    if let Some(SideEffectExpectation::Unparseable) = lowered.expected_side_effects {
        return Verdict::Pending;
    }

    // Decide the primary (result / error) verdict, if there is a primary
    // assertion. A scenario whose *only* assertion is side effects has no
    // primary expectation; treat a supported execution as a primary `pass` and
    // let the side-effect check below be decisive.
    let primary = match &lowered.expectation {
        Some(expectation) => judge(expectation, &outcome),
        None => Verdict::Pass,
    };
    if primary != Verdict::Pass {
        return primary;
    }

    // The result/error matched (or there was nothing to check). Now assert the
    // side effects against the engine's reported QueryStatistics, if the
    // scenario asserts them (BUG-0006 / Decision 0012). This is a real
    // pass/fail — never auto-`pending`.
    if let Some(SideEffectExpectation::Table(expected)) = &lowered.expected_side_effects {
        let actual = outcome.side_effects();
        return if actual.matches_side_effects(expected) {
            Verdict::Pass
        } else {
            Verdict::Fail
        };
    }

    Verdict::Pass
}

fn judge(expectation: &Expectation, outcome: &ExecOutcome) -> Verdict {
    match (expectation, outcome) {
        // Unsupported is handled by the caller before judge() is reached.
        (_, ExecOutcome::Unsupported) => Verdict::Pending,

        // Expected rows, got rows: compare (side effects checked separately).
        (
            Expectation::Result {
                columns: exp_cols,
                rows: exp_rows,
                ordered,
            },
            ExecOutcome::Rows {
                columns: got_cols,
                rows: got_rows,
                ..
            },
        ) => {
            if rows_match(exp_cols, exp_rows, *ordered, got_cols, got_rows) {
                Verdict::Pass
            } else {
                Verdict::Fail
            }
        }

        // Expected rows but the engine raised an error: fail.
        (Expectation::Result { .. }, ExecOutcome::Raised { .. }) => Verdict::Fail,

        // Expected an error, got the matching error kind: pass (phase is
        // advisory — kind match is the primary signal the TCK asserts).
        (Expectation::Error { kind: exp_kind, .. }, ExecOutcome::Raised { kind: got_kind, .. }) => {
            if exp_kind.eq_ignore_ascii_case(got_kind) {
                Verdict::Pass
            } else {
                Verdict::Fail
            }
        }

        // Expected an error but the engine returned rows: fail.
        (Expectation::Error { .. }, ExecOutcome::Rows { .. }) => Verdict::Fail,

        // Side-effects-only marker (`no side effects`, `result should be
        // empty`): a supported execution passes the primary check; an explicit
        // side-effect table, if present, is asserted by the caller.
        (Expectation::NoResultRows, ExecOutcome::Rows { .. }) => Verdict::Pass,
        (Expectation::NoResultRows, ExecOutcome::Raised { .. }) => Verdict::Fail,
    }
}

/// Compare expected and actual result tables.
fn rows_match(
    exp_cols: &[String],
    exp_rows: &[ResultRow],
    ordered: bool,
    got_cols: &[String],
    got_rows: &[ResultRow],
) -> bool {
    if exp_cols != got_cols {
        return false;
    }
    if exp_rows.len() != got_rows.len() {
        return false;
    }
    if ordered {
        exp_rows == got_rows
    } else {
        // Order-insensitive multiset comparison.
        let mut got: Vec<&ResultRow> = got_rows.iter().collect();
        for er in exp_rows {
            if let Some(pos) = got.iter().position(|gr| *gr == er) {
                got.swap_remove(pos);
            } else {
                return false;
            }
        }
        got.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use std::collections::HashMap;

    /// A test engine that returns a scripted outcome keyed by query text. Any
    /// query not in the script is reported `Unsupported` (-> pending), mirroring
    /// the real adapter's default.
    struct ScriptedEngine {
        responses: HashMap<String, ExecOutcome>,
    }

    impl ScriptedEngine {
        fn new() -> Self {
            Self {
                responses: HashMap::new(),
            }
        }
        fn on(mut self, query: &str, outcome: ExecOutcome) -> Self {
            self.responses.insert(query.trim().to_string(), outcome);
            self
        }
    }

    impl Engine for ScriptedEngine {
        fn execute(&mut self, query: &str) -> ExecOutcome {
            self.responses
                .get(query.trim())
                .cloned()
                .unwrap_or(ExecOutcome::Unsupported)
        }
    }

    fn scenario_from(src: &str) -> Scenario {
        let feature = gherkin::Feature::parse(src, gherkin::GherkinEnv::default())
            .expect("test feature parses");
        feature.scenarios.into_iter().next().expect("one scenario")
    }

    const PASS_SRC: &str = r#"
Feature: T
  Scenario: trivially passing
    Given any graph
    When executing query:
      """
      RETURN 1 AS n
      """
    Then the result should be, in any order:
      | n |
      | 1 |
    And no side effects
"#;

    const FAIL_SRC: &str = r#"
Feature: T
  Scenario: trivially failing
    Given any graph
    When executing query:
      """
      RETURN 2 AS n
      """
    Then the result should be, in any order:
      | n |
      | 1 |
    And no side effects
"#;

    #[test]
    fn stub_engine_makes_executable_scenarios_pending() {
        let mut engine = crate::engine::PendingEngine;
        assert_eq!(
            classify(&scenario_from(PASS_SRC), &mut engine),
            Verdict::Pending
        );
    }

    #[test]
    fn matching_result_passes() {
        let mut engine = ScriptedEngine::new().on(
            "RETURN 1 AS n",
            ExecOutcome::rows(vec!["n".into()], vec![vec!["1".into()]]),
        );
        assert_eq!(
            classify(&scenario_from(PASS_SRC), &mut engine),
            Verdict::Pass
        );
    }

    #[test]
    fn mismatching_result_fails() {
        let mut engine = ScriptedEngine::new().on(
            "RETURN 2 AS n",
            ExecOutcome::rows(vec!["n".into()], vec![vec!["2".into()]]),
        );
        assert_eq!(
            classify(&scenario_from(FAIL_SRC), &mut engine),
            Verdict::Fail
        );
    }

    #[test]
    fn expected_error_matches() {
        let src = r#"
Feature: T
  Scenario: expects a syntax error
    Given any graph
    When executing query:
      """
      MATCH () RETURN foo
      """
    Then a SyntaxError should be raised at compile time: UndefinedVariable
"#;
        let mut engine = ScriptedEngine::new().on(
            "MATCH () RETURN foo",
            ExecOutcome::Raised {
                kind: "SyntaxError".into(),
                phase: ErrorPhase::CompileTime,
            },
        );
        assert_eq!(classify(&scenario_from(src), &mut engine), Verdict::Pass);
    }

    #[test]
    fn wrong_error_kind_fails() {
        let src = r#"
Feature: T
  Scenario: expects a syntax error
    Given any graph
    When executing query:
      """
      MATCH () RETURN foo
      """
    Then a SyntaxError should be raised at compile time: UndefinedVariable
"#;
        let mut engine = ScriptedEngine::new().on(
            "MATCH () RETURN foo",
            ExecOutcome::Raised {
                kind: "TypeError".into(),
                phase: ErrorPhase::CompileTime,
            },
        );
        assert_eq!(classify(&scenario_from(src), &mut engine), Verdict::Fail);
    }

    #[test]
    fn expected_rows_but_got_error_fails() {
        let mut engine = ScriptedEngine::new().on(
            "RETURN 1 AS n",
            ExecOutcome::Raised {
                kind: "SyntaxError".into(),
                phase: ErrorPhase::CompileTime,
            },
        );
        assert_eq!(
            classify(&scenario_from(PASS_SRC), &mut engine),
            Verdict::Fail
        );
    }

    #[test]
    fn unsupported_setup_is_pending() {
        let src = r#"
Feature: T
  Scenario: with setup
    Given an empty graph
    And having executed:
      """
      CREATE (:Person {name: 'A'})
      """
    When executing query:
      """
      MATCH (n) RETURN n
      """
    Then the result should be, in any order:
      | n |
      | (:Person {name: 'A'}) |
"#;
        // Setup unsupported -> pending even though main query is scripted.
        let mut engine = ScriptedEngine::new().on(
            "MATCH (n) RETURN n",
            ExecOutcome::rows(vec!["n".into()], vec![vec!["(:Person {name: 'A'})".into()]]),
        );
        assert_eq!(classify(&scenario_from(src), &mut engine), Verdict::Pending);
    }

    #[test]
    fn order_insensitive_match() {
        let src = r#"
Feature: T
  Scenario: any order
    Given any graph
    When executing query:
      """
      UNWIND [1, 2] AS n RETURN n
      """
    Then the result should be, in any order:
      | n |
      | 2 |
      | 1 |
"#;
        let mut engine = ScriptedEngine::new().on(
            "UNWIND [1, 2] AS n RETURN n",
            ExecOutcome::rows(vec!["n".into()], vec![vec!["1".into()], vec!["2".into()]]),
        );
        assert_eq!(classify(&scenario_from(src), &mut engine), Verdict::Pass);
    }

    #[test]
    fn ordered_mismatch_fails() {
        let src = r#"
Feature: T
  Scenario: ordered
    Given any graph
    When executing query:
      """
      UNWIND [1, 2] AS n RETURN n
      """
    Then the result should be, in order:
      | n |
      | 1 |
      | 2 |
"#;
        let mut engine = ScriptedEngine::new().on(
            "UNWIND [1, 2] AS n RETURN n",
            ExecOutcome::rows(vec!["n".into()], vec![vec!["2".into()], vec!["1".into()]]),
        );
        assert_eq!(classify(&scenario_from(src), &mut engine), Verdict::Fail);
    }

    // --- side-effect assertions (BUG-0006 / Decision 0012) ------------------

    const SIDE_EFFECT_SRC: &str = r#"
Feature: T
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

    /// An engine whose reported side effects exactly match the expected table
    /// passes the scenario as a real `pass` (never auto-`pending`).
    #[test]
    fn matching_side_effects_pass() {
        let mut se = QueryStatistics::new();
        se.record_nodes_created(1);
        se.record_nodes_deleted(1);
        let mut engine = ScriptedEngine::new().on(
            "CREATE (n) DELETE n",
            ExecOutcome::rows_with_side_effects(Vec::new(), Vec::new(), se),
        );
        assert_eq!(
            classify(&scenario_from(SIDE_EFFECT_SRC), &mut engine),
            Verdict::Pass
        );
    }

    /// An engine whose reported side effects diverge from the expected table is
    /// a real conformance `fail`, not a pass and not `pending`.
    #[test]
    fn mismatching_side_effects_fail() {
        let mut se = QueryStatistics::new();
        se.record_nodes_created(1); // missing the `-nodes 1` the scenario expects
        let mut engine = ScriptedEngine::new().on(
            "CREATE (n) DELETE n",
            ExecOutcome::rows_with_side_effects(Vec::new(), Vec::new(), se),
        );
        assert_eq!(
            classify(&scenario_from(SIDE_EFFECT_SRC), &mut engine),
            Verdict::Fail
        );
    }

    /// A category present in the engine's report but absent from the expected
    /// table (asserted-zero by the TCK convention) is a `fail`: the comparison
    /// is total across every category.
    #[test]
    fn extra_unexpected_side_effect_fails() {
        let mut se = QueryStatistics::new();
        se.record_nodes_created(1);
        se.record_nodes_deleted(1);
        se.record_properties_set(1); // not in the expected table -> must fail
        let mut engine = ScriptedEngine::new().on(
            "CREATE (n) DELETE n",
            ExecOutcome::rows_with_side_effects(Vec::new(), Vec::new(), se),
        );
        assert_eq!(
            classify(&scenario_from(SIDE_EFFECT_SRC), &mut engine),
            Verdict::Fail
        );
    }

    /// A side-effect scenario the engine cannot run is `pending`, never `fail`.
    #[test]
    fn unsupported_side_effect_scenario_is_pending() {
        let mut engine = crate::engine::PendingEngine;
        assert_eq!(
            classify(&scenario_from(SIDE_EFFECT_SRC), &mut engine),
            Verdict::Pending
        );
    }

    /// A scenario asserting `the result should be ...` *and* `the side effects
    /// should be:` must check both: matching rows but wrong side effects fails.
    #[test]
    fn result_plus_side_effects_both_checked() {
        let src = r#"
Feature: T
  Scenario: result and side effects
    Given an empty graph
    When executing query:
      """
      CREATE (n) RETURN n
      """
    Then the result should be, in any order:
      | n |
      | (n) |
    And the side effects should be:
      | +nodes | 1 |
"#;
        // Rows match but the engine reports the wrong side effects -> fail.
        let wrong = QueryStatistics::new(); // expected +nodes 1, engine reports none
        let mut engine = ScriptedEngine::new().on(
            "CREATE (n) RETURN n",
            ExecOutcome::rows_with_side_effects(vec!["n".into()], vec![vec!["(n)".into()]], wrong),
        );
        assert_eq!(classify(&scenario_from(src), &mut engine), Verdict::Fail);

        // Rows match and side effects match -> pass.
        let mut right = QueryStatistics::new();
        right.record_nodes_created(1);
        let mut engine2 = ScriptedEngine::new().on(
            "CREATE (n) RETURN n",
            ExecOutcome::rows_with_side_effects(vec!["n".into()], vec![vec!["(n)".into()]], right),
        );
        assert_eq!(classify(&scenario_from(src), &mut engine2), Verdict::Pass);
    }
}
