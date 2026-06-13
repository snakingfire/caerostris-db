//! Expanding `Scenario Outline` blocks into one concrete scenario per
//! `Examples` data row (BUG-0009 / Decision 0013).
//!
//! The `gherkin` 0.16 parser captures a `Scenario Outline` once, with its
//! `<placeholder>` tokens still literal and its `Examples` tables attached via
//! [`Scenario::examples`]. It does **not** expand them. Counting each outline
//! once made the Cat. 4 (GATE) denominator ~2.4× too small (1602 vs the
//! conventional fully-expanded ~3880 at tag `2024.3`) and, worse, handed the
//! engine a query string with literal `<comp>` / `<boolop>` tokens — a false
//! `fail` (syntax error) or a permanent `pending` (`Unsupported`) the moment a
//! real engine plugged in. Decision 0008 forbids that curated-subset framing of
//! the GATE.
//!
//! [`expand_scenario`] performs the openCypher-conventional expansion the TCK
//! reference runner does:
//!
//! - A **plain** `Scenario:` (no `Examples`) yields exactly itself.
//! - A `Scenario Outline:` yields one scenario per `Examples` *data* row (every
//!   row after the header) across every `Examples:` block, with each
//!   `<header>` token substituted by that row's cell value in the scenario
//!   name, every step `value`, every step docstring, and every step data-table
//!   cell. Substitution is a **single simultaneous pass** — exactly the Gherkin
//!   `Scenario Outline` contract — so a placeholder embedded in a larger token
//!   (e.g. `'<text>'` or `{num: <num>}`) substitutes correctly, while a column
//!   value that itself contains a sibling column's `<token>` is placed verbatim
//!   and **never re-scanned** for further substitution (BUG-0021).
//! - A `Scenario Outline:` with no usable `Examples` rows yields **nothing**:
//!   an outline with zero variants has zero executable test cases (it cannot run
//!   with literal `<placeholder>` text), so it must not inflate the denominator
//!   with an unrunnable scenario.

use gherkin::{Scenario, Step, Table};

/// Expand one parsed Gherkin scenario into the concrete scenarios the TCK
/// executes.
///
/// See the module docs for the exact contract. The returned scenarios carry no
/// `examples` (each is already a concrete instance) and have every
/// `<placeholder>` substituted from its originating `Examples` data row.
#[must_use]
pub fn expand_scenario(scenario: &Scenario) -> Vec<Scenario> {
    // A plain scenario (no Examples) is its own single test case.
    if scenario.examples.is_empty() {
        return vec![scenario.clone()];
    }

    let mut expanded = Vec::new();
    for examples in &scenario.examples {
        let Some(table) = &examples.table else {
            continue;
        };
        let mut rows = table.rows.iter();
        // The first row is the header naming the `<placeholders>`; the rest are
        // data rows, each producing one concrete scenario.
        let Some(header) = rows.next() else {
            continue;
        };
        for data_row in rows {
            // Pair each header name with this row's cell. A short row (fewer
            // cells than headers) simply leaves the missing placeholders
            // unsubstituted, mirroring lenient Gherkin behaviour.
            let bindings: Vec<(String, &str)> = header
                .iter()
                .zip(data_row.iter())
                .map(|(h, v)| (format!("<{}>", h.trim()), v.as_str()))
                .collect();
            expanded.push(instantiate(scenario, &bindings));
        }
    }
    expanded
}

/// Build a concrete scenario from an outline + one row's `<placeholder>`
/// bindings. Clones the outline, substitutes every binding into the name and
/// each step, and clears `examples` so the result is a plain scenario.
fn instantiate(outline: &Scenario, bindings: &[(String, &str)]) -> Scenario {
    let mut concrete = outline.clone();
    concrete.name = substitute(&concrete.name, bindings);
    concrete.examples = Vec::new();
    for step in &mut concrete.steps {
        substitute_step(step, bindings);
    }
    concrete
}

/// Substitute every `<placeholder>` binding into a step's value, docstring, and
/// data-table cells — every textual surface a TCK placeholder can appear in.
fn substitute_step(step: &mut Step, bindings: &[(String, &str)]) {
    step.value = substitute(&step.value, bindings);
    if let Some(doc) = &step.docstring {
        step.docstring = Some(substitute(doc, bindings));
    }
    if let Some(table) = &mut step.table {
        *table = Table {
            rows: table
                .rows
                .iter()
                .map(|row| row.iter().map(|cell| substitute(cell, bindings)).collect())
                .collect(),
            span: table.span,
            position: table.position,
        };
    }
}

/// Replace every `<placeholder>` token in `text` with its bound value, in a
/// **single simultaneous pass** (Cucumber/Gherkin `Scenario Outline` semantics).
///
/// The scanner walks `text` left to right; at each `<...>` span it resolves the
/// token to its column value and copies that value **verbatim** into the output
/// — the cursor then advances past the original token, so a value that itself
/// contains a `<sibling>` token is never re-scanned for further substitution
/// (BUG-0021). An unbound `<token>` (no matching column header) and a malformed
/// `<` with no closing `>` are copied through verbatim, mirroring lenient
/// Gherkin behaviour. A placeholder can appear anywhere, including inside a
/// larger literal like `'<text>'` or `{num: <num>}`.
fn substitute(text: &str, bindings: &[(String, &str)]) -> String {
    // Nothing to do when there are no `<` markers at all — the overwhelmingly
    // common case for already-substituted or placeholder-free text.
    if !text.contains('<') {
        return text.to_string();
    }

    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        // Copy everything up to (and not including) the `<`.
        out.push_str(&rest[..open]);
        let after_open = &rest[open..]; // starts with '<'
        if let Some(close_rel) = after_open.find('>') {
            // `token` is the whole `<...>` span including the delimiters; this
            // is exactly the key form stored in `bindings`.
            let token = &after_open[..=close_rel];
            if let Some((_, value)) = bindings.iter().find(|(t, _)| t == token) {
                out.push_str(value); // verbatim; not re-scanned
            } else {
                out.push_str(token); // unbound: leave the literal `<token>`
            }
            rest = &after_open[close_rel + 1..];
        } else {
            // A dangling `<` with no `>`: copy the remainder verbatim and stop.
            out.push_str(after_open);
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use gherkin::{Feature, GherkinEnv, StepType};

    fn parse_one(src: &str) -> Scenario {
        Feature::parse(src, GherkinEnv::default())
            .expect("test feature parses")
            .scenarios
            .into_iter()
            .next()
            .expect("one scenario")
    }

    const OUTLINE_SRC: &str = r#"
Feature: T
  Scenario Outline: sort by <sort>
    Given an empty graph
    When executing query:
      """
      MATCH (a) RETURN a ORDER BY <sort>
      """
    Then the result should be, in any order:
      | a       |
      | <sort>  |
    Examples:
      | sort   |
      | a.num  |
      | a.name |
"#;

    #[test]
    fn plain_scenario_yields_itself() {
        let src = r#"
Feature: T
  Scenario: plain
    Given any graph
    When executing query:
      """
      RETURN 1 AS n
      """
    Then the result should be, in any order:
      | n |
      | 1 |
"#;
        let s = parse_one(src);
        let expanded = expand_scenario(&s);
        assert_eq!(expanded.len(), 1, "a plain scenario is one test case");
        assert_eq!(expanded[0], s, "expansion must not mutate a plain scenario");
    }

    #[test]
    fn outline_expands_to_one_scenario_per_data_row() {
        let s = parse_one(OUTLINE_SRC);
        let expanded = expand_scenario(&s);
        assert_eq!(
            expanded.len(),
            2,
            "two Examples data rows -> two concrete scenarios"
        );
    }

    #[test]
    fn placeholders_substituted_in_query_docstring() {
        let expanded = expand_scenario(&parse_one(OUTLINE_SRC));
        let queries: Vec<String> = expanded
            .iter()
            .map(|s| {
                s.steps
                    .iter()
                    .find(|st| st.ty == StepType::When)
                    .and_then(|st| st.docstring.clone())
                    .expect("when step has a docstring")
            })
            .collect();
        assert!(
            queries.iter().all(|q| !q.contains('<')),
            "no <placeholder> may survive expansion: {queries:?}"
        );
        assert!(queries.iter().any(|q| q.contains("ORDER BY a.num")));
        assert!(queries.iter().any(|q| q.contains("ORDER BY a.name")));
    }

    #[test]
    fn placeholders_substituted_in_result_table_cells() {
        let expanded = expand_scenario(&parse_one(OUTLINE_SRC));
        let table_cells: Vec<String> = expanded
            .iter()
            .flat_map(|s| {
                s.steps
                    .iter()
                    .filter_map(|st| st.table.as_ref())
                    .flat_map(|t| t.rows.iter().flatten().cloned())
            })
            .collect();
        assert!(
            table_cells.iter().all(|c| !c.contains('<')),
            "result-table placeholders must be substituted: {table_cells:?}"
        );
        assert!(table_cells.iter().any(|c| c == "a.num"));
        assert!(table_cells.iter().any(|c| c == "a.name"));
    }

    #[test]
    fn placeholder_substituted_in_scenario_name() {
        let expanded = expand_scenario(&parse_one(OUTLINE_SRC));
        let names: Vec<&str> = expanded.iter().map(|s| s.name.as_str()).collect();
        assert!(names.iter().all(|n| !n.contains('<')));
        assert!(names.contains(&"sort by a.num"));
        assert!(names.contains(&"sort by a.name"));
    }

    #[test]
    fn expanded_scenarios_carry_no_examples() {
        let expanded = expand_scenario(&parse_one(OUTLINE_SRC));
        assert!(
            expanded.iter().all(|s| s.examples.is_empty()),
            "each expanded scenario is concrete and must carry no Examples"
        );
    }

    #[test]
    fn placeholder_embedded_in_a_larger_literal_substitutes() {
        // A placeholder can appear inside a quoted/braced literal, e.g.
        // `'<text>'` or `{num: <num>}`. Substitution is textual.
        let src = r#"
Feature: T
  Scenario Outline: embedded
    Given any graph
    When executing query:
      """
      RETURN {num: <num>, text: '<text>'} AS v
      """
    Then the result should be, in any order:
      | v                              |
      | ({num: <num>, text: '<text>'}) |
    Examples:
      | num | text  |
      | 1   | hello |
"#;
        let expanded = expand_scenario(&parse_one(src));
        assert_eq!(expanded.len(), 1);
        let query = expanded[0]
            .steps
            .iter()
            .find(|st| st.ty == StepType::When)
            .and_then(|st| st.docstring.clone())
            .unwrap();
        // The parser keeps the docstring's surrounding newlines; the harness
        // trims them at classify time (see `scenario::lower`). Compare trimmed.
        assert_eq!(query.trim(), "RETURN {num: 1, text: 'hello'} AS v");
        let cell = &expanded[0]
            .steps
            .iter()
            .find_map(|st| st.table.as_ref())
            .unwrap()
            .rows[1][0];
        assert_eq!(cell, "({num: 1, text: 'hello'})");
    }

    #[test]
    fn multiple_examples_blocks_each_contribute_rows() {
        // The TCK occasionally splits variants across several `Examples:` blocks.
        let src = r#"
Feature: T
  Scenario Outline: multi <x>
    Given any graph
    When executing query:
      """
      RETURN <x> AS n
      """
    Then the result should be, in any order:
      | n   |
      | <x> |
    Examples:
      | x |
      | 1 |
      | 2 |
    Examples:
      | x |
      | 3 |
"#;
        let expanded = expand_scenario(&parse_one(src));
        assert_eq!(
            expanded.len(),
            3,
            "rows from every Examples block are expanded"
        );
        let names: Vec<&str> = expanded.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"multi 1"));
        assert!(names.contains(&"multi 2"));
        assert!(names.contains(&"multi 3"));
    }

    #[test]
    fn substitution_is_single_pass_not_chained() {
        // BUG-0021: a column value that *contains* a sibling column's
        // `<token>` must be placed verbatim and never re-scanned. Columns
        // `a`, `b` with data row `a = "<b>"`, `b = "x"`, query
        // `RETURN <a> AND <b>` must expand to `RETURN <b> AND x` — Cucumber's
        // single simultaneous-pass semantics. The buggy chained `String::replace`
        // produces `RETURN x AND x`: `<a>` -> `<b>`, then the later `<b>`
        // binding re-substitutes the just-injected `<b>` to `x`. Corrupt.
        let src = r#"
Feature: T
  Scenario Outline: chained <a> <b>
    Given any graph
    When executing query:
      """
      RETURN <a> AND <b>
      """
    Then the result should be, in any order:
      | n               |
      | <a> AND <b>     |
    Examples:
      | a   | b |
      | <b> | x |
"#;
        let expanded = expand_scenario(&parse_one(src));
        assert_eq!(expanded.len(), 1);
        let s = &expanded[0];

        // Scenario name: `<a>` -> literal `<b>`, original `<b>` -> `x`.
        assert_eq!(
            s.name, "chained <b> x",
            "scenario name must use single-pass substitution"
        );

        // Query docstring: the just-injected `<b>` must NOT be re-substituted.
        let query = s
            .steps
            .iter()
            .find(|st| st.ty == StepType::When)
            .and_then(|st| st.docstring.clone())
            .expect("when step has a docstring");
        assert_eq!(
            query.trim(),
            "RETURN <b> AND x",
            "single-pass: injected <b> from column `a` is verbatim, not re-substituted to `x`"
        );

        // Result-table cell: same single-pass invariant on every textual surface.
        let cell = &s
            .steps
            .iter()
            .find_map(|st| st.table.as_ref())
            .expect("then step has a table")
            .rows[1][0];
        assert_eq!(
            cell, "<b> AND x",
            "result-table cell must use single-pass substitution"
        );
    }

    #[test]
    fn substring_collision_between_column_names_substitutes_correctly() {
        // BUG-0021 note: column names that are substrings of each other (e.g.
        // `n` vs `name`) are unaffected, because the `<...>` delimiters make
        // `<n>` a non-substring of `<name>`. A single-pass scanner must keep
        // this correct: `<n>` -> its value, `<name>` -> its (different) value.
        let src = r#"
Feature: T
  Scenario Outline: collide <n> <name>
    Given any graph
    When executing query:
      """
      RETURN <n> AS short, <name> AS long
      """
    Then the result should be, in any order:
      | short | long   |
      | <n>   | <name> |
    Examples:
      | n | name  |
      | 1 | alice |
"#;
        let expanded = expand_scenario(&parse_one(src));
        assert_eq!(expanded.len(), 1);
        let query = expanded[0]
            .steps
            .iter()
            .find(|st| st.ty == StepType::When)
            .and_then(|st| st.docstring.clone())
            .unwrap();
        assert_eq!(query.trim(), "RETURN 1 AS short, alice AS long");
        assert_eq!(expanded[0].name, "collide 1 alice");
    }

    #[test]
    fn unbound_placeholder_is_left_verbatim() {
        // A `<token>` with no matching column header is left untouched by a
        // single-pass scan (lenient Gherkin behaviour: missing bindings are not
        // an error). The value `1` for `bound` is placed; `<unbound>` survives.
        let src = r#"
Feature: T
  Scenario Outline: keep <bound>
    Given any graph
    When executing query:
      """
      RETURN <bound>, <unbound>
      """
    Then the result should be, in any order:
      | n |
      | 1 |
    Examples:
      | bound |
      | 1     |
"#;
        let expanded = expand_scenario(&parse_one(src));
        assert_eq!(expanded.len(), 1);
        let query = expanded[0]
            .steps
            .iter()
            .find(|st| st.ty == StepType::When)
            .and_then(|st| st.docstring.clone())
            .unwrap();
        assert_eq!(
            query.trim(),
            "RETURN 1, <unbound>",
            "an unbound placeholder is left verbatim by the single-pass scan"
        );
    }

    #[test]
    fn longest_placeholder_token_wins_at_an_offset() {
        // Defensive: ensure the one-pass scanner anchors on `<` and reads to the
        // matching `>`, so `<x>` inside the text is matched as the whole token
        // and not confused with a shorter prefix. Two distinct tokens back to
        // back substitute independently in a single pass.
        let src = r#"
Feature: T
  Scenario Outline: adjacent <x><y>
    Given any graph
    When executing query:
      """
      RETURN '<x><y>'
      """
    Then the result should be, in any order:
      | n |
      | 1 |
    Examples:
      | x  | y  |
      | <y>| ab |
"#;
        let expanded = expand_scenario(&parse_one(src));
        assert_eq!(expanded.len(), 1);
        let query = expanded[0]
            .steps
            .iter()
            .find(|st| st.ty == StepType::When)
            .and_then(|st| st.docstring.clone())
            .unwrap();
        // `<x>` -> literal `<y>` (verbatim, not re-substituted); `<y>` -> `ab`.
        assert_eq!(query.trim(), "RETURN '<y>ab'");
    }

    #[test]
    fn outline_with_no_examples_rows_yields_nothing() {
        // A header-only `Examples:` block has zero data rows: zero runnable
        // variants. It must not leak an unrunnable `<placeholder>` scenario into
        // the denominator.
        let src = r#"
Feature: T
  Scenario Outline: empty <x>
    Given any graph
    When executing query:
      """
      RETURN <x> AS n
      """
    Then the result should be, in any order:
      | n   |
      | <x> |
    Examples:
      | x |
"#;
        let expanded = expand_scenario(&parse_one(src));
        assert!(
            expanded.is_empty(),
            "an outline with no data rows has no executable test case"
        );
    }
}
