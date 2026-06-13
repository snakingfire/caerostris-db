//! openCypher TCK pass-rate contract: pinned suite identity + a non-gameable
//! pass-rate definition.
//!
//! This module encodes the **rules** that the live TCK harness (`T-0002`) must
//! obey when it reports the Cat. 4 GATE metric. It deliberately does *not*
//! parse `.feature` files or execute Cypher — that is the harness's job. It
//! exists so the definition of "100% of the TCK" is fixed in code and tested,
//! and so the harness and the `rubric-grader` cron consume the same constants.
//!
//! ## Why this exists (BUG-0007)
//!
//! Cat. 4 of the master rubric is a GATE scored as the TCK pass-rate, with a
//! 100% bar. Two ambiguities made "100%" gameable:
//!
//! 1. **The denominator.** "pass-rate" plus a separate `pending` bucket invites
//!    the back-door reading `pass_rate = pass / (pass + fail)`, which excludes
//!    unimplemented (`pending`) scenarios from the denominator. That is a
//!    curated subset by another name and a falsification of "100% means all of
//!    it, not a subset" (`docs/commanders-intent.md`). We mandate
//!    [`pass_rate`] `= pass / total`, with `total = pass + pending + fail`, so
//!    incompleteness is always visible in the score.
//! 2. **Suite identity.** "100% of the TCK" is meaningless without a pinned
//!    release tag and a recorded scenario count; otherwise the rate can rise by
//!    silently dropping `.feature` files. We pin a specific release
//!    ([`PINNED_TCK_TAG`] at [`PINNED_TCK_COMMIT`]) and record its measured
//!    scenario count ([`PINNED_TCK_SCENARIOS`]). [`verify_suite_size`] fails the
//!    build if the harness loads a different count.
//!
//! See `.project/decisions/0008-tck-passrate-definition-and-pinning.md` and
//! `docs/requirements/master-rubric.md` (Cat. 4).

use core::fmt;

/// The pinned openCypher TCK release tag. Grading is reproducible only against a
/// fixed suite; tracking a moving branch cannot anchor a GATE. Bumping this pin
/// is a deliberate, recorded action (update the constants below and Decision
/// 0008 in the same change).
pub const PINNED_TCK_TAG: &str = "1.0.0-M23";

/// The exact commit the pinned tag resolves to in
/// `github.com/opencypher/openCypher`. Recorded so the pin survives a tag being
/// re-pointed upstream.
pub const PINNED_TCK_COMMIT: &str = "007895aff5f33097d67b2e48a0a2babd6bd18590";

/// Number of `.feature` files in `tck/features/` at the pinned tag. Recorded as
/// a secondary integrity signal (a dropped file usually drops many scenarios).
pub const PINNED_TCK_FEATURE_FILES: usize = 220;

/// Total Gherkin scenarios (`Scenario:` + `Scenario Outline:`) across all
/// `.feature` files at the pinned tag. This is the canonical `total` for the
/// Cat. 4 metric and the rubric's suite-integrity check: the harness MUST load
/// exactly this many scenarios (see [`verify_suite_size`]).
///
/// Measured directly at the pinned commit:
/// `grep -rhE '^\s*(Scenario|Scenario Outline):' tck/features --include='*.feature' | wc -l`
/// → 1339 `Scenario:` + 276 `Scenario Outline:` = 1615.
pub const PINNED_TCK_SCENARIOS: usize = 1615;

/// The non-gameable TCK pass-rate: `pass / total`, where
/// `total = pass + pending + fail`.
///
/// Both `pending` (unimplemented) and `fail` (wrong result) sit in the
/// denominator, so an incomplete engine can never read as 100%. An empty suite
/// returns `0.0` (not `NaN`), so a missing/unloaded suite scores as the floor
/// rather than silently passing.
///
/// ```
/// // 50 pass, 50 pending, 0 fail: the rate is 0.5, not 1.0.
/// assert!((caerostris_db::tck::pass_rate(50, 50, 0) - 0.5).abs() < f64::EPSILON);
/// assert_eq!(caerostris_db::tck::pass_rate(0, 0, 0), 0.0);
/// ```
#[must_use]
pub fn pass_rate(pass: usize, pending: usize, fail: usize) -> f64 {
    let total = pass + pending + fail;
    if total == 0 {
        return 0.0;
    }
    // usize -> f64 is lossless for any scenario count we will ever see
    // (well under 2^53); the cast is exact for the TCK.
    pass as f64 / total as f64
}

/// A machine-readable TCK run summary — the shape the harness emits to
/// `.project/reports/tck-latest.json` and the `rubric-grader` consumes.
///
/// Carries the pinned tag and commit alongside the counts so the grader can
/// assert the suite was not shrunk before trusting the rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TckSummary {
    /// Scenarios whose result matched the expected `Then` outcome.
    pub pass: usize,
    /// Scenarios skipped because the engine does not yet implement the feature.
    /// Counted toward `total` — never excluded to inflate the rate.
    pub pending: usize,
    /// Scenarios that ran but produced the wrong result (a real defect).
    pub fail: usize,
}

impl TckSummary {
    /// Build a summary from the three buckets.
    #[must_use]
    pub fn new(pass: usize, pending: usize, fail: usize) -> Self {
        Self {
            pass,
            pending,
            fail,
        }
    }

    /// `pass + pending + fail` — the denominator of the rate. No scenario is
    /// excluded.
    #[must_use]
    pub fn total(&self) -> usize {
        self.pass + self.pending + self.fail
    }

    /// `pass / total`. See [`pass_rate`].
    #[must_use]
    pub fn pass_rate(&self) -> f64 {
        pass_rate(self.pass, self.pending, self.fail)
    }

    /// True only when the suite is fully green: `pending == 0 && fail == 0`.
    /// This — not `pass_rate == 1.0` over a curated subset — is the Cat. 4
    /// "100%" bar.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.pending == 0 && self.fail == 0
    }

    /// Serialize to the documented JSON shape (stable field order, no external
    /// dependency). `pass_rate` is rendered with fixed precision so the grader
    /// can parse it deterministically.
    ///
    /// ```json
    /// {"tck_tag":"1.0.0-M23","tck_commit":"007895a…","total":1615,
    ///  "pass":0,"pending":1615,"fail":0,"pass_rate":0.000000}
    /// ```
    #[must_use]
    pub fn to_json(&self) -> String {
        format!(
            concat!(
                "{{\"tck_tag\":\"{tag}\",\"tck_commit\":\"{commit}\",",
                "\"total\":{total},\"pass\":{pass},\"pending\":{pending},",
                "\"fail\":{fail},\"pass_rate\":{rate:.6}}}"
            ),
            tag = PINNED_TCK_TAG,
            commit = PINNED_TCK_COMMIT,
            total = self.total(),
            pass = self.pass,
            pending = self.pending,
            fail = self.fail,
            rate = self.pass_rate(),
        )
    }
}

/// Error returned by [`verify_suite_size`] when the loaded scenario count does
/// not match the pinned total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuiteSizeError {
    /// The count the harness was required to load ([`PINNED_TCK_SCENARIOS`]).
    pub expected: usize,
    /// The count actually loaded from the vendored `.feature` files.
    pub loaded: usize,
}

impl fmt::Display for SuiteSizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TCK suite size mismatch: loaded {loaded} scenarios, but the pinned \
             release {tag} ({commit}) has {expected}. The suite was shrunk, \
             grown, or the wrong tag is checked out — grading is not reproducible \
             until this matches.",
            loaded = self.loaded,
            expected = self.expected,
            tag = PINNED_TCK_TAG,
            commit = PINNED_TCK_COMMIT,
        )
    }
}

impl std::error::Error for SuiteSizeError {}

/// Guard against silent suite drift: the number of scenarios the harness loaded
/// must equal [`PINNED_TCK_SCENARIOS`]. Returns `Err` otherwise so the harness
/// (and CI) refuse to report a pass-rate over a tampered suite.
///
/// # Errors
///
/// Returns [`SuiteSizeError`] if `loaded != PINNED_TCK_SCENARIOS`.
pub fn verify_suite_size(loaded: usize) -> Result<(), SuiteSizeError> {
    if loaded == PINNED_TCK_SCENARIOS {
        Ok(())
    } else {
        Err(SuiteSizeError {
            expected: PINNED_TCK_SCENARIOS,
            loaded,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_excludes_nothing_from_the_denominator() {
        assert!((pass_rate(1, 1, 1) - (1.0 / 3.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn empty_is_floor_not_nan() {
        assert_eq!(pass_rate(0, 0, 0), 0.0);
        assert!(!pass_rate(0, 0, 0).is_nan());
    }

    #[test]
    fn all_pass_is_one() {
        assert!((pass_rate(7, 0, 0) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn complete_only_with_zero_pending_and_fail() {
        assert!(TckSummary::new(3, 0, 0).is_complete());
        assert!(!TckSummary::new(3, 1, 0).is_complete());
        assert!(!TckSummary::new(3, 0, 1).is_complete());
        // The degenerate empty suite is "complete" in the boolean sense but its
        // rate is 0.0 and its total is 0 — the suite-size guard rejects it.
        assert!(verify_suite_size(0).is_err());
    }

    #[test]
    fn json_is_stable_and_contains_pin() {
        let s = TckSummary::new(0, PINNED_TCK_SCENARIOS, 0);
        let j = s.to_json();
        assert_eq!(
            j,
            "{\"tck_tag\":\"1.0.0-M23\",\
             \"tck_commit\":\"007895aff5f33097d67b2e48a0a2babd6bd18590\",\
             \"total\":1615,\"pass\":0,\"pending\":1615,\"fail\":0,\
             \"pass_rate\":0.000000}"
        );
    }

    #[test]
    fn guard_matches_only_exact_count() {
        assert!(verify_suite_size(PINNED_TCK_SCENARIOS).is_ok());
        assert!(verify_suite_size(PINNED_TCK_SCENARIOS - 1).is_err());
        assert!(verify_suite_size(PINNED_TCK_SCENARIOS + 1).is_err());
    }

    #[test]
    fn suite_size_error_display_is_actionable() {
        let e = verify_suite_size(10).unwrap_err();
        let msg = e.to_string();
        assert!(msg.contains("1615"));
        assert!(msg.contains("1.0.0-M23"));
        assert!(msg.contains("10"));
    }
}
