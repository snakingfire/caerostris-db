//! Integration tests for the openCypher TCK pass-rate contract (BUG-0007).
//!
//! These tests pin the *definition* of the Cat. 4 GATE metric so it cannot be
//! gamed. They assert:
//!
//! 1. `pass_rate = pass / total`, with `total = pass + pending + fail` — both
//!    `pending` and `fail` are in the denominator (no curated subset).
//! 2. Reaching 100% requires `pending == 0 && fail == 0`.
//! 3. The pinned TCK release tag and its expected scenario count are recorded
//!    and emitted in the machine-readable summary.
//! 4. A guard rejects a loaded scenario count that differs from the pinned total
//!    (catches silent suite shrinkage).
//!
//! See `.project/decisions/0008-tck-passrate-definition-and-pinning.md`.

use caerostris_db::tck;

#[test]
fn pending_is_in_the_denominator() {
    // 50 pass, 50 pending, 0 fail. The gameable reading pass/(pass+fail) = 1.0
    // would hide the 50 unimplemented scenarios. The correct reading is 0.5.
    let pr = tck::pass_rate(50, 50, 0);
    assert!(
        (pr - 0.5).abs() < f64::EPSILON,
        "pending must depress the rate: expected 0.5, got {pr}"
    );
}

#[test]
fn fail_is_in_the_denominator() {
    let pr = tck::pass_rate(50, 0, 50);
    assert!((pr - 0.5).abs() < f64::EPSILON, "got {pr}");
}

#[test]
fn empty_suite_is_zero_not_nan() {
    assert_eq!(tck::pass_rate(0, 0, 0), 0.0);
}

#[test]
fn full_pass_requires_no_pending_and_no_fail() {
    assert!(tck::TckSummary::new(tck::PINNED_TCK_SCENARIOS, 0, 0).is_complete());
    // A single pending scenario is not 100%, even though pass/(pass+fail) == 1.0.
    let one_pending = tck::TckSummary::new(tck::PINNED_TCK_SCENARIOS - 1, 1, 0);
    assert!(!one_pending.is_complete());
    let one_fail = tck::TckSummary::new(tck::PINNED_TCK_SCENARIOS - 1, 0, 1);
    assert!(!one_fail.is_complete());
}

#[test]
fn summary_total_sums_the_three_buckets() {
    let s = tck::TckSummary::new(10, 20, 30);
    assert_eq!(s.total(), 60);
    assert!((s.pass_rate() - (10.0 / 60.0)).abs() < f64::EPSILON);
}

#[test]
fn pinned_tag_and_count_are_recorded() {
    assert_eq!(tck::PINNED_TCK_TAG, "1.0.0-M23");
    assert_eq!(
        tck::PINNED_TCK_COMMIT,
        "007895aff5f33097d67b2e48a0a2babd6bd18590"
    );
    assert_eq!(tck::PINNED_TCK_FEATURE_FILES, 220);
    assert_eq!(tck::PINNED_TCK_SCENARIOS, 1615);
}

#[test]
fn machine_readable_summary_emits_tag_and_total() {
    let json = tck::TckSummary::new(0, tck::PINNED_TCK_SCENARIOS, 0).to_json();
    // The grader and the suite-shrinkage check both depend on these fields.
    assert!(json.contains("\"tck_tag\":\"1.0.0-M23\""), "json: {json}");
    assert!(
        json.contains("\"tck_commit\":\"007895aff5f33097d67b2e48a0a2babd6bd18590\""),
        "json: {json}"
    );
    assert!(json.contains("\"total\":1615"), "json: {json}");
    assert!(json.contains("\"pass\":0"), "json: {json}");
    assert!(json.contains("\"pending\":1615"), "json: {json}");
    assert!(json.contains("\"fail\":0"), "json: {json}");
    assert!(json.contains("\"pass_rate\":0"), "json: {json}");
}

#[test]
fn suite_size_guard_accepts_the_pinned_count() {
    assert!(tck::verify_suite_size(tck::PINNED_TCK_SCENARIOS).is_ok());
}

#[test]
fn suite_size_guard_rejects_shrunken_suite() {
    // Dropping even one scenario must fail the guard.
    let err = tck::verify_suite_size(tck::PINNED_TCK_SCENARIOS - 1)
        .expect_err("a shrunken suite must be rejected");
    assert_eq!(err.expected, tck::PINNED_TCK_SCENARIOS);
    assert_eq!(err.loaded, tck::PINNED_TCK_SCENARIOS - 1);
}

#[test]
fn suite_size_guard_rejects_grown_suite() {
    // An unexpectedly larger suite (wrong tag checked out) must also fail.
    assert!(tck::verify_suite_size(tck::PINNED_TCK_SCENARIOS + 1).is_err());
}
