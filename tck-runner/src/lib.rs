//! openCypher TCK conformance runner for caerostris-db.
//!
//! Cat. 4 of the [master rubric] scores the project at the **live TCK
//! pass-rate**, with a hard floor of `0` if the harness is not wired. This crate
//! wires it: it parses the vendored openCypher TCK Gherkin corpus
//! (`tck/openCypher/features`, pinned at tag `2024.3`), drives each scenario
//! against the caerostris-db engine through the [`engine::Engine`] adapter, and
//! emits a machine-readable [`report::Report`] the rubric grader consumes.
//!
//! Until the openCypher engine lands (EPIC-002), the default
//! [`engine::PendingEngine`] reports every query as unsupported, so every
//! scenario is counted `pending` (never `fail`). The pass-rate therefore starts
//! at `0.0` over a real denominator and climbs as language features land.
//!
//! ```
//! use tck_runner::engine::PendingEngine;
//! use tck_runner::runner::run_suite;
//! use std::path::Path;
//!
//! // An empty/missing corpus yields an all-zero summary rather than panicking.
//! let summary = run_suite(Path::new("/no/such/corpus"), || PendingEngine).unwrap();
//! assert_eq!(summary.total, 0);
//! assert_eq!(summary.pass_rate(), 0.0);
//! ```
//!
//! [master rubric]: ../../docs/requirements/master-rubric.md

pub mod engine;
pub mod outline;
pub mod report;
pub mod runner;
pub mod scenario;

use std::path::{Path, PathBuf};

use crate::report::Provenance;

/// Path (relative to the workspace root) of the vendored TCK feature corpus.
pub const DEFAULT_FEATURES_SUBDIR: &str = "tck/openCypher/features";

/// Best-effort resolution of the vendored TCK corpus directory.
///
/// Resolves `<workspace-root>/tck/openCypher/features` by walking up from this
/// crate's manifest directory (this crate lives at `<root>/tck-runner`). Falls
/// back to the path relative to the current working directory.
#[must_use]
pub fn default_features_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = <workspace-root>/tck-runner ; parent = workspace root.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(workspace_root) = manifest_dir.parent() {
        let candidate = workspace_root.join(DEFAULT_FEATURES_SUBDIR);
        if candidate.is_dir() {
            return candidate;
        }
    }
    PathBuf::from(DEFAULT_FEATURES_SUBDIR)
}

/// Read the corpus [`Provenance`] (pinned tag + commit) recorded next to a
/// features directory — i.e. in its parent (`tck/openCypher/PINNED_TAG`,
/// `PINNED_COMMIT`). Missing markers yield empty fields rather than an error.
#[must_use]
pub fn read_provenance(features_dir: &Path) -> Provenance {
    let base = features_dir.parent().unwrap_or(features_dir);
    let read_trimmed = |name: &str| {
        std::fs::read_to_string(base.join(name))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    Provenance {
        tck_tag: read_trimmed("PINNED_TAG"),
        pinned_commit: read_trimmed("PINNED_COMMIT"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_features_dir_points_at_vendored_corpus() {
        let dir = default_features_dir();
        assert!(
            dir.ends_with(DEFAULT_FEATURES_SUBDIR),
            "unexpected corpus path: {}",
            dir.display()
        );
    }

    #[test]
    fn read_provenance_picks_up_pinned_markers() {
        // The vendored corpus records its pinned tag + commit.
        let prov = read_provenance(&default_features_dir());
        assert_eq!(prov.tck_tag.as_deref(), Some("2024.3"));
        assert!(
            prov.pinned_commit.is_some_and(|c| c.len() >= 7),
            "pinned commit should be recorded"
        );
    }

    #[test]
    fn read_provenance_is_empty_when_markers_absent() {
        let prov = read_provenance(Path::new("/nonexistent/features"));
        assert!(prov.tck_tag.is_none());
        assert!(prov.pinned_commit.is_none());
    }
}
