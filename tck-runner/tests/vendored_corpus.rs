//! Integration tests over the real vendored openCypher TCK corpus.
//!
//! These assert the corpus is present, parses, has the expected scenario count
//! for the pinned release (`2024.3`), records its provenance, and — under the
//! stub engine — yields only `pending` verdicts (zero unexpected `fail`s). This
//! is the live evidence for board item `T-0002` acceptance criteria, and the
//! suite-shrinkage guard required by BUG-0007.

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

/// Feature files the current Gherkin parser (`gherkin` 0.16) cannot parse —
/// `Literals6.feature` only, due to its heavily-escaped result-table cells.
/// Tracked by BUG-0008; until fixed these files land in `parse_errors`, never
/// in `pending`/`fail`, so they cannot inflate the pass-rate.
const KNOWN_UNPARSEABLE_FILES: usize = 1;

/// Scenarios in the known-unparseable file(s) (`Literals6` has 13).
const SCENARIOS_IN_UNPARSEABLE_FILES: usize = 13;

/// Scenarios the harness currently parses + counts.
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
