//! The machine-readable summary the rubric grader consumes (Cat. 4).
//!
//! After a full run the harness emits a JSON object of the shape (counts shown
//! for the pinned `2024.3` corpus under the stub engine: `total` is the
//! *expanded* test-case count — Scenario Outlines expanded per `Examples` row,
//! BUG-0009 — and `parse_errors` is the one BUG-0018 unparseable file,
//! `Literals6.feature`):
//!
//! ```json
//! {
//!   "total": 3884,
//!   "pass": 0,
//!   "pending": 3884,
//!   "fail": 0,
//!   "parse_errors": 1,
//!   "pass_rate": 0.0
//! }
//! ```
//!
//! `pass_rate` is `pass / total` in `[0.0, 1.0]`; the grader reads it directly
//! as the Cat. 4 score (× 100 for a percentage).

use serde::Serialize;

use crate::scenario::Verdict;

/// Aggregate counts over a TCK run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Summary {
    /// Total scenarios discovered across all parsed feature files.
    pub total: usize,
    /// Scenarios the engine ran and matched the expectation.
    pub pass: usize,
    /// Scenarios that use a construct the engine does not yet support.
    pub pending: usize,
    /// Scenarios the engine ran but mismatched the expectation.
    pub fail: usize,
    /// Feature files that failed to parse (a harness/corpus problem, surfaced
    /// separately so it never silently inflates `pending`).
    pub parse_errors: usize,
}

impl Summary {
    /// Record one scenario verdict.
    pub fn record(&mut self, verdict: Verdict) {
        self.total += 1;
        match verdict {
            Verdict::Pass => self.pass += 1,
            Verdict::Pending => self.pending += 1,
            Verdict::Fail => self.fail += 1,
        }
    }

    /// Fraction of total scenarios that passed, in `[0.0, 1.0]`.
    /// Returns `0.0` when there are no scenarios (avoids a divide-by-zero and
    /// keeps Cat. 4 at its floor rather than `NaN`).
    #[must_use]
    pub fn pass_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.pass as f64 / self.total as f64
        }
    }

    /// Fold another summary into this one (used to combine per-file results).
    pub fn merge(&mut self, other: &Summary) {
        self.total += other.total;
        self.pass += other.pass;
        self.pending += other.pending;
        self.fail += other.fail;
        self.parse_errors += other.parse_errors;
    }
}

/// Where the corpus came from, so the grader can detect silent suite shrinkage
/// (per BUG-0007: a pinned tag + recorded `total` are the suite's integrity
/// check). `None` fields mean the corpus had no provenance markers.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Provenance {
    /// The openCypher release tag the corpus is pinned to (e.g. `"2024.3"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tck_tag: Option<String>,
    /// The exact upstream commit the corpus was vendored from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned_commit: Option<String>,
}

/// JSON-serializable view including the derived `pass_rate` and corpus
/// provenance.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub total: usize,
    pub pass: usize,
    pub pending: usize,
    pub fail: usize,
    pub parse_errors: usize,
    pub pass_rate: f64,
    #[serde(flatten)]
    pub provenance: Provenance,
}

impl From<&Summary> for Report {
    fn from(s: &Summary) -> Self {
        Report::with_provenance(s, Provenance::default())
    }
}

impl Report {
    /// Build a report from a summary plus corpus provenance.
    #[must_use]
    pub fn with_provenance(s: &Summary, provenance: Provenance) -> Self {
        Report {
            total: s.total,
            pass: s.pass,
            pending: s.pending,
            fail: s.fail,
            parse_errors: s.parse_errors,
            pass_rate: s.pass_rate(),
            provenance,
        }
    }
}

impl Report {
    /// Render the report as pretty JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("Report serializes to JSON")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_summary_has_zero_pass_rate_not_nan() {
        let s = Summary::default();
        assert_eq!(s.pass_rate(), 0.0);
        assert!(!s.pass_rate().is_nan());
    }

    #[test]
    fn records_each_verdict() {
        let mut s = Summary::default();
        s.record(Verdict::Pass);
        s.record(Verdict::Pass);
        s.record(Verdict::Pending);
        s.record(Verdict::Fail);
        assert_eq!(s.total, 4);
        assert_eq!(s.pass, 2);
        assert_eq!(s.pending, 1);
        assert_eq!(s.fail, 1);
        assert!((s.pass_rate() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn merge_adds_componentwise() {
        let mut a = Summary {
            total: 2,
            pass: 1,
            pending: 1,
            fail: 0,
            parse_errors: 0,
        };
        let b = Summary {
            total: 3,
            pass: 0,
            pending: 2,
            fail: 1,
            parse_errors: 1,
        };
        a.merge(&b);
        assert_eq!(a.total, 5);
        assert_eq!(a.pass, 1);
        assert_eq!(a.pending, 3);
        assert_eq!(a.fail, 1);
        assert_eq!(a.parse_errors, 1);
    }

    #[test]
    fn report_json_contains_all_fields() {
        let s = Summary {
            total: 10,
            pass: 4,
            pending: 5,
            fail: 1,
            parse_errors: 0,
        };
        let json = Report::from(&s).to_json();
        for field in [
            "total",
            "pass",
            "pending",
            "fail",
            "parse_errors",
            "pass_rate",
        ] {
            assert!(json.contains(field), "missing field {field} in {json}");
        }
        // Round-trips back to the same numbers.
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["total"], 10);
        assert_eq!(parsed["pass"], 4);
        assert!((parsed["pass_rate"].as_f64().unwrap() - 0.4).abs() < 1e-9);
    }
}
