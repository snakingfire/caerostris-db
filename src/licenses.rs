//! License-manifest hygiene checks (rubric Cat. 12).
//!
//! Every dependency that ends up in `Cargo.lock` must be recorded in the
//! committed license manifest at `docs/licenses/manifest.toml`, with a
//! permissive SPDX identifier from an approved allow-list. This module provides
//! the pure parsing/checking logic; an integration test
//! (`tests/license_manifest.rs`) runs it against the real repository files so a
//! dependency added without a manifest entry — or carrying a non-permissive
//! license — fails CI.
//!
//! The check is intentionally self-contained (no `cargo-deny` needed at runtime)
//! so it works in any environment. `cargo-deny` is *additionally* wired into CI
//! (`deny.toml`) as defense in depth — see `docs/process/open-source-guardrails.md`.

use std::collections::BTreeSet;

/// SPDX identifiers permitted without steering sign-off.
///
/// Mirrors the "approved license families" list in
/// `docs/process/open-source-guardrails.md`. Copyleft / source-available
/// licenses are deliberately excluded: they require a recorded steering
/// decision before use and must not pass this automated check silently.
pub const APPROVED_SPDX: &[&str] = &[
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "MPL-2.0",
    "CC0-1.0",
    "Unlicense",
    "Unicode-3.0",
    "Zlib",
];

/// A single dependency parsed out of `Cargo.lock`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LockedCrate {
    /// Crate name as it appears in `Cargo.lock`.
    pub name: String,
    /// Resolved version.
    pub version: String,
}

/// A recorded entry in the license manifest (`docs/licenses/manifest.toml`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    /// Crate/package name.
    pub name: String,
    /// Recorded SPDX license expression (single-license form for the allow-list
    /// check; OR-expressions are split on `OR`).
    pub spdx: String,
}

/// A problem found while checking the lockfile against the manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LicenseViolation {
    /// A dependency in `Cargo.lock` has no entry in the manifest.
    MissingManifestEntry { crate_name: String },
    /// A manifest entry carries an SPDX id outside the approved allow-list.
    NonPermissiveLicense { crate_name: String, spdx: String },
}

impl std::fmt::Display for LicenseViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LicenseViolation::MissingManifestEntry { crate_name } => write!(
                f,
                "dependency `{crate_name}` is in Cargo.lock but missing from \
                 docs/licenses/manifest.toml — add an entry with its SPDX id \
                 and a permissive-compatibility note"
            ),
            LicenseViolation::NonPermissiveLicense { crate_name, spdx } => write!(
                f,
                "dependency `{crate_name}` is recorded with license `{spdx}`, \
                 which is not in the approved allow-list — get steering sign-off \
                 and record the decision before adding it"
            ),
        }
    }
}

/// Returns `true` if every license token in an SPDX expression is approved.
///
/// Handles simple `A OR B` / `A/B` (legacy crates.io) disjunctions: a crate is
/// acceptable if *any* offered license is approved. AND-style expressions
/// (`A AND B`) require *all* tokens to be approved; we treat any non-OR
/// separator conservatively as requiring all tokens.
#[must_use]
pub fn is_permissive(spdx: &str) -> bool {
    let normalized = spdx.replace('/', " OR ");
    if normalized.to_ascii_uppercase().contains(" OR ") {
        // Disjunction: any approved license suffices.
        normalized
            .split(" OR ")
            .map(|tok| tok.trim().trim_matches(|c| c == '(' || c == ')').trim())
            .any(|tok| APPROVED_SPDX.iter().any(|a| a.eq_ignore_ascii_case(tok)))
    } else if normalized.to_ascii_uppercase().contains(" AND ") {
        // Conjunction: all components must be approved.
        normalized
            .split(" AND ")
            .map(|tok| tok.trim().trim_matches(|c| c == '(' || c == ')').trim())
            .all(|tok| APPROVED_SPDX.iter().any(|a| a.eq_ignore_ascii_case(tok)))
    } else {
        let tok = normalized.trim();
        APPROVED_SPDX.iter().any(|a| a.eq_ignore_ascii_case(tok))
    }
}

/// Parse the crate entries out of a `Cargo.lock` file body.
///
/// `own_crates` are names of workspace members (e.g. the crate itself), which
/// are skipped — they are not third-party dependencies needing a license entry.
#[must_use]
pub fn parse_lockfile(contents: &str, own_crates: &[&str]) -> Vec<LockedCrate> {
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    let mut in_package = false;

    let flush =
        |name: &mut Option<String>, version: &mut Option<String>, out: &mut Vec<LockedCrate>| {
            if let (Some(n), Some(v)) = (name.take(), version.take()) {
                if !own_crates.contains(&n.as_str()) {
                    out.push(LockedCrate {
                        name: n,
                        version: v,
                    });
                }
            } else {
                *name = None;
                *version = None;
            }
        };

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            // Starting a new package block — flush any complete previous one.
            flush(&mut name, &mut version, &mut out);
            in_package = true;
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("name = ") {
            name = Some(rest.trim_matches('"').to_string());
        } else if let Some(rest) = trimmed.strip_prefix("version = ") {
            version = Some(rest.trim_matches('"').to_string());
        }
    }
    // Flush the final package block.
    flush(&mut name, &mut version, &mut out);
    out
}

/// Parse the license manifest body.
///
/// The manifest is a TOML-ish table list, but to avoid a TOML dependency we
/// parse the small, well-defined subset we write ourselves: blocks introduced
/// by `[[dependency]]` with `name = "..."` and `spdx = "..."` keys.
#[must_use]
pub fn parse_manifest(contents: &str) -> Vec<ManifestEntry> {
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    let mut spdx: Option<String> = None;
    let mut in_dep = false;

    let flush =
        |name: &mut Option<String>, spdx: &mut Option<String>, out: &mut Vec<ManifestEntry>| {
            if let (Some(n), Some(s)) = (name.clone(), spdx.clone()) {
                out.push(ManifestEntry { name: n, spdx: s });
            }
            *name = None;
            *spdx = None;
        };

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed == "[[dependency]]" {
            flush(&mut name, &mut spdx, &mut out);
            in_dep = true;
            continue;
        }
        if !in_dep {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("name = ") {
            name = Some(rest.trim_matches('"').to_string());
        } else if let Some(rest) = trimmed.strip_prefix("spdx = ") {
            spdx = Some(rest.trim_matches('"').to_string());
        }
    }
    flush(&mut name, &mut spdx, &mut out);
    out
}

/// Check every locked dependency against the manifest and the allow-list.
///
/// Returns the (possibly empty) set of violations. An empty result means the
/// manifest is complete and every recorded license is permissive.
#[must_use]
pub fn check(locked: &[LockedCrate], manifest: &[ManifestEntry]) -> Vec<LicenseViolation> {
    let mut violations = Vec::new();

    let manifest_names: BTreeSet<&str> = manifest.iter().map(|e| e.name.as_str()).collect();

    for dep in locked {
        if !manifest_names.contains(dep.name.as_str()) {
            violations.push(LicenseViolation::MissingManifestEntry {
                crate_name: dep.name.clone(),
            });
        }
    }

    for entry in manifest {
        if !is_permissive(&entry.spdx) {
            violations.push(LicenseViolation::NonPermissiveLicense {
                crate_name: entry.name.clone(),
                spdx: entry.spdx.clone(),
            });
        }
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approved_single_licenses_are_permissive() {
        assert!(is_permissive("MIT"));
        assert!(is_permissive("Apache-2.0"));
        assert!(is_permissive("BSD-3-Clause"));
        assert!(is_permissive("MPL-2.0"));
    }

    #[test]
    fn copyleft_licenses_are_not_permissive() {
        assert!(!is_permissive("GPL-3.0"));
        assert!(!is_permissive("AGPL-3.0"));
        assert!(!is_permissive("LGPL-2.1"));
        assert!(!is_permissive("SSPL-1.0"));
        assert!(!is_permissive("BUSL-1.1"));
    }

    #[test]
    fn or_expression_is_permissive_if_any_token_approved() {
        assert!(is_permissive("MIT OR Apache-2.0"));
        assert!(is_permissive("GPL-3.0 OR MIT"));
        assert!(is_permissive("MIT/Apache-2.0")); // legacy slash form
    }

    #[test]
    fn or_expression_is_not_permissive_if_no_token_approved() {
        assert!(!is_permissive("GPL-3.0 OR AGPL-3.0"));
    }

    #[test]
    fn and_expression_requires_all_tokens_approved() {
        assert!(is_permissive("MIT AND Apache-2.0"));
        assert!(!is_permissive("MIT AND GPL-3.0"));
    }

    #[test]
    fn parse_lockfile_skips_own_crate() {
        let lock = r#"
version = 4

[[package]]
name = "caerostris-db"
version = "0.0.0"
"#;
        let parsed = parse_lockfile(lock, &["caerostris-db"]);
        assert!(parsed.is_empty(), "own crate should be skipped: {parsed:?}");
    }

    #[test]
    fn parse_lockfile_extracts_dependencies() {
        let lock = r#"
version = 4

[[package]]
name = "caerostris-db"
version = "0.0.0"
dependencies = [
 "serde",
]

[[package]]
name = "serde"
version = "1.0.210"

[[package]]
name = "thiserror"
version = "1.0.64"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;
        let parsed = parse_lockfile(lock, &["caerostris-db"]);
        assert_eq!(
            parsed,
            vec![
                LockedCrate {
                    name: "serde".into(),
                    version: "1.0.210".into()
                },
                LockedCrate {
                    name: "thiserror".into(),
                    version: "1.0.64".into()
                },
            ]
        );
    }

    #[test]
    fn parse_manifest_extracts_entries() {
        let manifest = r#"
# License manifest
[[dependency]]
name = "serde"
version = "1.0.210"
spdx = "MIT OR Apache-2.0"
note = "Permissive; ubiquitous."

[[dependency]]
name = "thiserror"
version = "1.0.64"
spdx = "MIT OR Apache-2.0"
note = "Permissive."
"#;
        let parsed = parse_manifest(manifest);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "serde");
        assert_eq!(parsed[0].spdx, "MIT OR Apache-2.0");
        assert_eq!(parsed[1].name, "thiserror");
    }

    #[test]
    fn check_flags_dependency_missing_from_manifest() {
        let locked = vec![LockedCrate {
            name: "serde".into(),
            version: "1.0.210".into(),
        }];
        let manifest = vec![]; // empty — serde not recorded
        let violations = check(&locked, &manifest);
        assert_eq!(
            violations,
            vec![LicenseViolation::MissingManifestEntry {
                crate_name: "serde".into()
            }]
        );
    }

    #[test]
    fn check_flags_non_permissive_manifest_entry() {
        let locked = vec![LockedCrate {
            name: "copyleft-crate".into(),
            version: "1.0.0".into(),
        }];
        let manifest = vec![ManifestEntry {
            name: "copyleft-crate".into(),
            spdx: "GPL-3.0".into(),
        }];
        let violations = check(&locked, &manifest);
        assert_eq!(
            violations,
            vec![LicenseViolation::NonPermissiveLicense {
                crate_name: "copyleft-crate".into(),
                spdx: "GPL-3.0".into()
            }]
        );
    }

    #[test]
    fn check_passes_when_all_deps_recorded_and_permissive() {
        let locked = vec![LockedCrate {
            name: "serde".into(),
            version: "1.0.210".into(),
        }];
        let manifest = vec![ManifestEntry {
            name: "serde".into(),
            spdx: "MIT OR Apache-2.0".into(),
        }];
        assert!(check(&locked, &manifest).is_empty());
    }

    #[test]
    fn violation_display_is_actionable() {
        let v = LicenseViolation::MissingManifestEntry {
            crate_name: "serde".into(),
        };
        let msg = v.to_string();
        assert!(msg.contains("serde"));
        assert!(msg.contains("manifest.toml"));
    }
}
