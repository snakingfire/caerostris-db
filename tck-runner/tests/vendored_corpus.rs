//! Integration tests over the real vendored openCypher TCK corpus.
//!
//! These assert the corpus is present, parses, has the expected scenario count
//! for the pinned release (`2024.3`), records its provenance, and — under the
//! stub engine — yields only `pending` verdicts (zero unexpected `fail`s). This
//! is the live evidence for board item `T-0002` acceptance criteria, and the
//! suite-shrinkage guard required by BUG-0007.
//!
//! # Two named, guarded denominator gaps
//!
//! Both are honestly recorded so the Cat. 4 (GATE) denominator cannot silently
//! shift to certify a false 100% (Decision 0008 forbids any curated-subset
//! framing):
//!
//! - **BUG-0008 (Literals6):** one feature file the `gherkin` 0.16 parser
//!   cannot read; its 13 scenarios land in `parse_errors`, never `pending`/`fail`.
//! - **BUG-0009 (Scenario Outline expansion):** the harness counts each
//!   `Scenario Outline` **once**, not once per `Examples` data row. The
//!   conventional fully-expanded openCypher count at 2024.3 is ~3880; the
//!   harness counts 1602. Under the stub engine every outline is `pending`, so
//!   the current `0/1602` baseline is internally consistent and honest — but a
//!   real engine (EPIC-002) must expand outlines first, or the achievable
//!   pass-rate is silently capped below 100%. The guard
//!   [`outline_expansion_gap_is_named_and_guarded`] pins the unexpanded counts
//!   so this choice is explicit, not silent. See
//!   `.project/decisions/0013-tck-scenario-outline-expansion-gap.md` and BUG-0009.

use tck_runner::engine::PendingEngine;
use tck_runner::report::Report;
use tck_runner::runner::{discover_features, run_suite};
use tck_runner::{default_features_dir, read_provenance};

/// The number of `.feature` files vendored at openCypher tag `2024.3`.
const EXPECTED_FEATURE_FILES: usize = 220;

/// The official scenario count (Scenario + Scenario Outline, counted once each)
/// at tag `2024.3`, verified against
/// `grep -rhE '^\s*(Scenario|Scenario Outline):'` over `tck/features`.
const OFFICIAL_SCENARIOS: usize = 1615;

/// Plain `Scenario:` definitions at tag `2024.3`
/// (`grep -rhE '^\s*Scenario:'`).
const PLAIN_SCENARIOS: usize = 1339;

/// `Scenario Outline:` definitions at tag `2024.3`
/// (`grep -rhE '^\s*Scenario Outline:'`). Each is counted **once** today
/// (BUG-0009): outlines are not yet expanded per `Examples` row.
const SCENARIO_OUTLINES: usize = 276;

/// `Examples:` data rows across all outlines at tag `2024.3`. The conventional
/// fully-expanded scenario count is `PLAIN_SCENARIOS + EXAMPLES_DATA_ROWS`
/// (= 3880); the harness does not yet reach this (BUG-0009).
const EXAMPLES_DATA_ROWS: usize = 2541;

/// Feature files the current Gherkin parser (`gherkin` 0.16) cannot parse —
/// `Literals6.feature` only, due to its heavily-escaped result-table cells.
/// Tracked by BUG-0008; until fixed these files land in `parse_errors`, never
/// in `pending`/`fail`, so they cannot inflate the pass-rate.
const KNOWN_UNPARSEABLE_FILES: usize = 1;

/// Scenarios in the known-unparseable file(s) (`Literals6` has 13).
const SCENARIOS_IN_UNPARSEABLE_FILES: usize = 13;

/// Scenarios the harness currently parses + counts (outlines counted once).
const EXPECTED_PARSEABLE_SCENARIOS: usize = OFFICIAL_SCENARIOS - SCENARIOS_IN_UNPARSEABLE_FILES;

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
fn corpus_parses_to_expected_counts() {
    let dir = default_features_dir();
    let summary = run_suite(&dir, || PendingEngine).expect("corpus runs");

    // Exactly the one known parser limitation (BUG-0008), no more.
    assert_eq!(
        summary.parse_errors, KNOWN_UNPARSEABLE_FILES,
        "unexpected number of unparseable feature files — see BUG-0008"
    );
    // The counted scenarios plus those stuck in the unparseable file must equal
    // the official total: the suite is fully accounted for, nothing silently
    // dropped or excluded from the denominator.
    assert_eq!(
        summary.total, EXPECTED_PARSEABLE_SCENARIOS,
        "parsed scenario count drifted from the pinned release 2024.3"
    );
    assert_eq!(
        summary.total + SCENARIOS_IN_UNPARSEABLE_FILES,
        OFFICIAL_SCENARIOS,
        "parsed + unparseable scenarios must equal the official TCK total"
    );
}

#[test]
fn stub_engine_yields_only_pending_no_unexpected_failures() {
    let dir = default_features_dir();
    let summary = run_suite(&dir, || PendingEngine).expect("corpus runs");

    // The stub engine supports nothing, so every parsed scenario must be
    // pending. Critically: zero fails — unimplemented is `pending`, never
    // `fail` (T-0002 acceptance criterion).
    assert_eq!(
        summary.fail, 0,
        "stub engine must never produce a hard fail"
    );
    assert_eq!(summary.pass, 0, "stub engine cannot pass any scenario");
    assert_eq!(
        summary.pending, summary.total,
        "every parsed scenario must be counted pending under the stub engine"
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
    assert_eq!(parsed["total"], EXPECTED_PARSEABLE_SCENARIOS);
    assert_eq!(parsed["fail"], 0);
    assert_eq!(parsed["parse_errors"], KNOWN_UNPARSEABLE_FILES);
    assert_eq!(parsed["pass_rate"].as_f64(), Some(0.0));
    assert_eq!(parsed["tck_tag"], "2024.3");
    assert!(parsed["pinned_commit"].is_string());
}

/// BUG-0009 guard: pin the unexpanded Scenario-Outline denominator so the
/// choice to count each outline **once** (rather than once per `Examples` row)
/// is explicit and cannot silently shift to certify a false 100% (Decision
/// 0008). Counts `Scenario:` / `Scenario Outline:` / `Examples` data rows
/// directly from the vendored corpus and reconciles them against the harness's
/// counted total.
///
/// When outline expansion lands (BUG-0009), this guard is updated alongside the
/// new expanded denominator — a deliberate, reviewed change, not a silent one.
#[test]
fn outline_expansion_gap_is_named_and_guarded() {
    let dir = default_features_dir();
    let (plain, outlines, example_rows) = count_gherkin_constructs(&dir);

    // The corpus matches the pinned-release composition.
    assert_eq!(
        plain, PLAIN_SCENARIOS,
        "plain `Scenario:` count drifted from pinned 2024.3"
    );
    assert_eq!(
        outlines, SCENARIO_OUTLINES,
        "`Scenario Outline:` count drifted from pinned 2024.3"
    );
    assert_eq!(
        example_rows, EXAMPLES_DATA_ROWS,
        "`Examples` data-row count drifted from pinned 2024.3"
    );

    // Official total = plain + outlines (each outline counted ONCE today).
    assert_eq!(
        plain + outlines,
        OFFICIAL_SCENARIOS,
        "plain + outline definitions must equal the official scenario count"
    );

    // The harness's counted total equals the unexpanded denominator minus the
    // BUG-0008 unparseable scenarios. This is the documented, honest baseline:
    // outlines are NOT yet expanded (BUG-0009), so `total` is the unexpanded
    // count, not the conventional ~3880 fully-expanded count.
    let summary = run_suite(&dir, || PendingEngine).expect("corpus runs");
    assert_eq!(
        summary.total,
        plain + outlines - SCENARIOS_IN_UNPARSEABLE_FILES,
        "harness total must equal unexpanded(plain+outlines) minus unparseable; \
         if this changed because outlines are now expanded, update this guard \
         and the documented denominator (BUG-0009)"
    );

    // Document the size of the gap so it is impossible to overlook: the
    // conventional fully-expanded count is materially larger than what we count.
    let fully_expanded = plain + example_rows;
    assert_eq!(
        fully_expanded, 3880,
        "fully-expanded count (plain + example rows) drifted from pinned 2024.3"
    );
    assert!(
        fully_expanded > summary.total,
        "the unexpanded denominator ({}) understates the fully-expanded count \
         ({}); the BUG-0009 gap must remain named while it is open",
        summary.total,
        fully_expanded
    );
}

/// Count `Scenario:`, `Scenario Outline:`, and `Examples` *data* rows across
/// every vendored `.feature` file. A data row is a `|`-delimited row inside an
/// `Examples:` block other than the first (header) row. Mirrors the
/// `grep`/manual counts recorded in BUG-0009.
fn count_gherkin_constructs(dir: &std::path::Path) -> (usize, usize, usize) {
    let (mut plain, mut outlines, mut example_rows) = (0usize, 0usize, 0usize);
    for path in discover_features(dir).expect("corpus is readable") {
        let text = std::fs::read_to_string(&path).expect("feature file is readable");
        let mut in_examples = false;
        let mut header_seen = false;
        for line in text.lines() {
            let s = line.trim();
            if s.starts_with("Scenario Outline:") {
                outlines += 1;
                in_examples = false;
            } else if s.starts_with("Scenario:") {
                plain += 1;
                in_examples = false;
            } else if s.starts_with("Examples:") {
                in_examples = true;
                header_seen = false;
            } else if in_examples {
                if s.starts_with('|') {
                    if header_seen {
                        example_rows += 1;
                    } else {
                        header_seen = true; // the header row, not a data row
                    }
                } else if !s.is_empty() {
                    in_examples = false;
                }
            }
        }
    }
    (plain, outlines, example_rows)
}
