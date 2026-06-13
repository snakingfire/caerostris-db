//! Integration tests over the real vendored openCypher TCK corpus.
//!
//! These assert the corpus is present, parses, expands to the expected
//! test-case count for the pinned release (`2024.3`), records its provenance,
//! and — under the stub engine — yields only `pending` verdicts (zero
//! unexpected `fail`s). This is the live evidence for board items `T-0002` and
//! `BUG-0009`, and the suite-shrinkage guard required by BUG-0007.
//!
//! # The denominator is fully accounted for
//!
//! Every scenario in the pinned suite is accounted for, so the Cat. 4 (GATE)
//! denominator cannot silently shift to certify a false 100% (Decision 0008
//! forbids any curated-subset framing):
//!
//! - **BUG-0018 (Literals6 parse gap):** one feature file
//!   (`expressions/literals/Literals6.feature`) the `gherkin` 0.16 parser cannot
//!   read; its 13 scenarios land in `parse_errors`, never `pending`/`fail`. Still
//!   open — its scenarios are excluded from `total` until it parses. Owned by
//!   BUG-0018; this gap was previously *mis-cited* as "BUG-0008", which is an
//!   unrelated SPDX license-classification bug. [`parse_gap_is_exactly_literals6`]
//!   names and guards the exact file.
//! - **BUG-0009 (Scenario Outline expansion) — RESOLVED.** The harness now
//!   expands each `Scenario Outline` into one concrete scenario per `Examples`
//!   data row, substituting `<placeholder>` tokens into the query, setup
//!   statements, and expected-result cells (see `tck_runner::outline`). `total`
//!   therefore reflects the conventional openCypher test-case count: the 1326
//!   parseable plain `Scenario:` definitions plus 2558 expanded outline cases =
//!   **3884**, rather than the old unexpanded 1602. The reconciliation guard
//!   [`outline_expansion_total_is_reconciled`] pins the expanded denominator (and
//!   asserts no `<placeholder>` survives expansion) so it cannot silently shift
//!   in either direction. See
//!   `.project/decisions/0013-tck-scenario-outline-expansion-gap.md` and BUG-0009.

use gherkin::{Feature, GherkinEnv};

use tck_runner::engine::PendingEngine;
use tck_runner::outline::expand_scenario;
use tck_runner::report::Report;
use tck_runner::runner::{discover_features, run_suite};
use tck_runner::{default_features_dir, read_provenance};

/// The number of `.feature` files vendored at openCypher tag `2024.3`.
const EXPECTED_FEATURE_FILES: usize = 220;

/// The official scenario *definition* count (`Scenario` + `Scenario Outline`,
/// counted once each) at tag `2024.3`, verified against
/// `grep -rhE '^\s*(Scenario|Scenario Outline):'` over `tck/features`. This is
/// the count of definitions, **not** executable test cases — outlines expand to
/// many cases (see [`EXPANDED_TOTAL`]).
const OFFICIAL_SCENARIO_DEFINITIONS: usize = 1615;

/// Plain `Scenario:` definitions across the whole corpus at tag `2024.3`
/// (`grep -rhE '^\s*Scenario:'`). 13 of these live in the unparseable
/// `Literals6` file (the parse gap owned by BUG-0018).
const PLAIN_SCENARIOS: usize = 1339;

/// `Scenario Outline:` definitions at tag `2024.3`
/// (`grep -rhE '^\s*Scenario Outline:'`). Each now expands per `Examples` data
/// row (BUG-0009); none live in the unparseable file.
const SCENARIO_OUTLINES: usize = 276;

/// Feature files the current Gherkin parser (`gherkin` 0.16) cannot parse —
/// `Literals6.feature` only, due to its heavily-escaped result-table cells.
/// Tracked by BUG-0018; until fixed these files land in `parse_errors`, never
/// in `pending`/`fail`, so they cannot inflate the pass-rate.
const KNOWN_UNPARSEABLE_FILES: usize = 1;

/// The exact corpus-relative path of the single unparseable feature file. Naming
/// it (not just counting it) is the BUG-0018 ownership requirement: a *new* file
/// silently failing to parse, or this gap silently closing, must fail CI rather
/// than slip by as an unexplained `parse_errors` delta.
const UNPARSEABLE_FEATURE_REL: &str = "expressions/literals/Literals6.feature";

/// Scenarios in the known-unparseable file(s) (`Literals6` has 13 plain
/// scenarios, 0 outlines).
const SCENARIOS_IN_UNPARSEABLE_FILES: usize = 13;

/// Parseable plain `Scenario:` definitions = all plain minus the 13 in the
/// unparseable `Literals6` file.
const PARSEABLE_PLAIN_SCENARIOS: usize = PLAIN_SCENARIOS - SCENARIOS_IN_UNPARSEABLE_FILES;

/// Concrete scenarios produced by expanding every `Scenario Outline:` over its
/// `Examples` data rows, as counted by the authoritative `gherkin` parser.
///
/// Note this is **2558**, not the 2541 a naive `grep` of `Examples` rows
/// reports: the grep heuristic mis-handles commented-out (`#| ... |`) example
/// rows in `expressions/precedence/Precedence1.feature`, prematurely ending the
/// table and dropping 17 real data rows. The parser is authoritative — it is
/// what actually drives the engine — so the harness expansion (and this guard)
/// use the parser's count.
const EXPANDED_OUTLINE_CASES: usize = 2558;

/// The full executable test-case count the harness reports as `total`:
/// parseable plain scenarios + expanded outline cases. This is the BUG-0009
/// expanded denominator (was 1602 when outlines were counted once).
const EXPANDED_TOTAL: usize = PARSEABLE_PLAIN_SCENARIOS + EXPANDED_OUTLINE_CASES;

#[test]
fn corpus_is_vendored_and_discoverable() {
    let dir = default_features_dir();
    assert!(
        dir.is_dir(),
        "vendored TCK corpus missing at {} — run scripts/tck/fetch.sh",
        dir.display()
    );
    let files = discover_features(&dir).expect("corpus is readable");
    // Guard against suite shrinkage (dropping .feature files to game the rate).
    assert_eq!(
        files.len(),
        EXPECTED_FEATURE_FILES,
        "vendored feature-file count drifted from pinned tag 2024.3"
    );
}

#[test]
fn corpus_records_its_pinned_provenance() {
    let prov = read_provenance(&default_features_dir());
    assert_eq!(
        prov.tck_tag.as_deref(),
        Some("2024.3"),
        "the pinned TCK tag must be recorded for the grader's integrity check"
    );
    assert!(
        prov.pinned_commit.is_some(),
        "the pinned upstream commit must be recorded"
    );
}

#[test]
fn corpus_expands_to_expected_total() {
    let dir = default_features_dir();
    let summary = run_suite(&dir, || PendingEngine).expect("corpus runs");

    // Exactly the one known parser limitation (BUG-0018), no more.
    assert_eq!(
        summary.parse_errors, KNOWN_UNPARSEABLE_FILES,
        "unexpected number of unparseable feature files — see BUG-0018"
    );
    // `total` is now the *expanded* test-case count: each Scenario Outline
    // contributes one case per Examples data row (BUG-0009). If this drifts,
    // either the corpus changed or expansion regressed — both must fail CI.
    assert_eq!(
        summary.total, EXPANDED_TOTAL,
        "expanded scenario count drifted from the pinned release 2024.3"
    );
}

/// BUG-0018 ownership guard: the parse-error gap is exactly one *named* file —
/// `expressions/literals/Literals6.feature` — not an anonymous `parse_errors`
/// count. This satisfies AC#1 (the exact file is identified) and AC#2 (the gap
/// is owned/tracked): if a different file starts failing, or this one starts
/// parsing, the count *and* the name diverge from the pin and CI fails, forcing
/// a deliberate, reviewed update rather than a silent denominator shift.
#[test]
fn parse_gap_is_exactly_literals6() {
    use tck_runner::runner::unparseable_features;
    let dir = default_features_dir();
    let unparseable = unparseable_features(&dir).expect("corpus is readable");

    assert_eq!(
        unparseable.len(),
        KNOWN_UNPARSEABLE_FILES,
        "the parse gap must be exactly {KNOWN_UNPARSEABLE_FILES} file(s); got {unparseable:?} \
         — a new unparseable file is a regression, file a BUG (cf. BUG-0018)"
    );
    assert!(
        unparseable[0].ends_with(UNPARSEABLE_FEATURE_REL),
        "the single unparseable file must be {UNPARSEABLE_FEATURE_REL} (owned by BUG-0018), \
         got {:?}",
        unparseable[0]
    );

    // The named file must actually exist in the vendored corpus (so the guard
    // cannot pass vacuously if the corpus is restructured).
    assert!(
        dir.join(UNPARSEABLE_FEATURE_REL).is_file(),
        "{UNPARSEABLE_FEATURE_REL} must exist in the vendored corpus"
    );

    // And the named-vs-counted views are consistent: the number of files the
    // helper *names* equals the number the suite *counts* as parse errors.
    let summary = run_suite(&dir, || PendingEngine).expect("corpus runs");
    assert_eq!(
        summary.parse_errors,
        unparseable.len(),
        "named parse-error files must equal the counted parse_errors"
    );
}

#[test]
fn stub_engine_yields_only_pending_no_unexpected_failures() {
    let dir = default_features_dir();
    let summary = run_suite(&dir, || PendingEngine).expect("corpus runs");

    // The stub engine supports nothing, so every expanded scenario must be
    // pending. Critically: zero fails — unimplemented is `pending`, never
    // `fail`. With expansion, the engine is handed substituted queries, so an
    // outline variant is `pending` (unsupported), never a false `fail` from a
    // literal `<placeholder>` (the BUG-0009 latent defect).
    assert_eq!(
        summary.fail, 0,
        "stub engine must never produce a hard fail"
    );
    assert_eq!(summary.pass, 0, "stub engine cannot pass any scenario");
    assert_eq!(
        summary.pending, summary.total,
        "every expanded scenario must be counted pending under the stub engine"
    );
    assert_eq!(summary.pass_rate(), 0.0);
}

#[test]
fn report_json_carries_counts_and_provenance() {
    let dir = default_features_dir();
    let summary = run_suite(&dir, || PendingEngine).expect("corpus runs");
    let report = Report::with_provenance(&summary, read_provenance(&dir));
    let json = report.to_json();

    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(parsed["total"], EXPANDED_TOTAL);
    assert_eq!(parsed["fail"], 0);
    assert_eq!(parsed["parse_errors"], KNOWN_UNPARSEABLE_FILES);
    assert_eq!(parsed["pass_rate"].as_f64(), Some(0.0));
    assert_eq!(parsed["tck_tag"], "2024.3");
    assert!(parsed["pinned_commit"].is_string());
}

/// BUG-0009 reconciliation guard: pin the *expanded* Scenario-Outline
/// denominator so the count cannot silently shift (in either direction) to
/// certify a false 100% (Decision 0008), and prove that expansion left no
/// `<placeholder>` token in any executable query or expected-result cell — the
/// exact latent defect (false `fail` / stuck `pending`) BUG-0009 names.
///
/// It re-derives, from the authoritative `gherkin` parser, the plain-scenario
/// and outline-definition composition and the expanded case count, reconciles
/// them against the pinned constants and the harness's reported `total`, and
/// scans every expanded scenario for surviving placeholders.
#[test]
fn outline_expansion_total_is_reconciled() {
    let dir = default_features_dir();
    let (plain, outlines, expanded_cases, placeholder_survivors) = walk_corpus(&dir);

    // Composition matches the pinned-release definition counts (parseable part).
    assert_eq!(
        plain, PARSEABLE_PLAIN_SCENARIOS,
        "parseable plain `Scenario:` count drifted from pinned 2024.3"
    );
    assert_eq!(
        outlines, SCENARIO_OUTLINES,
        "`Scenario Outline:` count drifted from pinned 2024.3"
    );
    assert_eq!(
        expanded_cases, EXPANDED_OUTLINE_CASES,
        "expanded outline-case count drifted from pinned 2024.3"
    );

    // Definition arithmetic: parseable + unparseable definitions = official.
    assert_eq!(
        plain + outlines + SCENARIOS_IN_UNPARSEABLE_FILES,
        OFFICIAL_SCENARIO_DEFINITIONS,
        "parseable + unparseable scenario *definitions* must equal the official count"
    );

    // The harness's reported `total` equals the expanded denominator and is
    // materially larger than the old unexpanded definition count (1602) — the
    // BUG-0009 fix. If outlines were ever counted once again, this fails.
    let summary = run_suite(&dir, || PendingEngine).expect("corpus runs");
    assert_eq!(
        summary.total,
        plain + expanded_cases,
        "harness total must equal parseable-plain + expanded-outline-cases (BUG-0009)"
    );
    assert_eq!(summary.total, EXPANDED_TOTAL);
    let old_unexpanded_total = OFFICIAL_SCENARIO_DEFINITIONS - SCENARIOS_IN_UNPARSEABLE_FILES; // 1602
    assert!(
        summary.total > old_unexpanded_total,
        "expanded total ({}) must exceed the old once-per-outline count ({})",
        summary.total,
        old_unexpanded_total
    );

    // No `<placeholder>` may survive expansion in any executable query or
    // expected-result cell: a survivor is the BUG-0009 latent defect.
    assert_eq!(
        placeholder_survivors, 0,
        "{placeholder_survivors} expanded scenario(s) still carry a literal \
         <placeholder> — substitution is incomplete (BUG-0009)"
    );
}

/// Walk the vendored corpus through the same `gherkin` parser the harness uses
/// and return `(parseable_plain, outline_definitions, expanded_outline_cases,
/// placeholder_survivors)`.
///
/// `placeholder_survivors` counts expanded scenarios whose query docstring or a
/// result-table cell still contains a `<...>` token after substitution.
fn walk_corpus(dir: &std::path::Path) -> (usize, usize, usize, usize) {
    let (mut plain, mut outlines, mut expanded_cases, mut survivors) = (0, 0, 0, 0);
    for path in discover_features(dir).expect("corpus is readable") {
        let Ok(feature) = Feature::parse_path(&path, GherkinEnv::default()) else {
            continue; // BUG-0018 unparseable file; accounted for separately.
        };
        let mut defs: Vec<&gherkin::Scenario> = feature.scenarios.iter().collect();
        for rule in &feature.rules {
            defs.extend(rule.scenarios.iter());
        }
        for def in defs {
            if def.examples.is_empty() {
                plain += 1;
            } else {
                outlines += 1;
            }
            for concrete in expand_scenario(def) {
                if !def.examples.is_empty() {
                    expanded_cases += 1;
                }
                if scenario_has_placeholder(&concrete) {
                    survivors += 1;
                }
            }
        }
    }
    (plain, outlines, expanded_cases, survivors)
}

/// True if any step's docstring or any data-table cell of `scenario` still
/// contains an unsubstituted `<placeholder>` token (a `<word>` form).
fn scenario_has_placeholder(scenario: &gherkin::Scenario) -> bool {
    fn looks_like_placeholder(text: &str) -> bool {
        // A `<name>` token: `<`, one-or-more non-`>` non-whitespace chars, `>`.
        let bytes = text.as_bytes();
        let mut i = 0;
        while let Some(open) = text[i..].find('<') {
            let start = i + open + 1;
            if let Some(close_rel) = text[start..].find('>') {
                let inner = &text[start..start + close_rel];
                if !inner.is_empty()
                    && !inner.contains(char::is_whitespace)
                    && inner.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    return true;
                }
                i = start + close_rel + 1;
            } else {
                break;
            }
        }
        let _ = bytes;
        false
    }
    scenario.steps.iter().any(|step| {
        step.docstring
            .as_deref()
            .is_some_and(looks_like_placeholder)
            || step.table.as_ref().is_some_and(|t| {
                t.rows
                    .iter()
                    .flatten()
                    .any(|cell| looks_like_placeholder(cell))
            })
    })
}
