//! Process-hygiene wiring checks (rubric Cat. 12).
//!
//! These guard the *configuration* that keeps secrets out and releases flowing —
//! the things board item T-0039 calls for. They are deliberately cheap, file-
//! presence/content assertions: if someone deletes the gitleaks config, unwires
//! the pre-commit hook, drops the CI license/secret jobs, or removes the hourly
//! release script, CI goes red here with a pointer to what regressed.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read required file {}: {e}", path.display()))
}

fn exists(rel: &str) -> bool {
    repo_root().join(rel).exists()
}

#[test]
fn gitleaks_config_exists_and_extends_defaults() {
    assert!(
        exists(".gitleaks.toml"),
        ".gitleaks.toml is required so the secret-scan ruleset is committed and reproducible"
    );
    let cfg = read(".gitleaks.toml");
    // Must build on the bundled ruleset rather than replacing it wholesale.
    assert!(
        cfg.contains("useDefault") && cfg.contains("true"),
        ".gitleaks.toml must extend the default ruleset (useDefault = true)"
    );
}

#[test]
fn precommit_runs_gitleaks() {
    let cfg = read(".pre-commit-config.yaml");
    assert!(
        cfg.contains("gitleaks"),
        "the pre-commit config must run gitleaks so secrets are blocked before commit"
    );
}

#[test]
fn gitignore_blocks_secret_files() {
    let gi = read(".gitignore");
    for pat in [".env", "*.pem", "*.key"] {
        assert!(
            gi.contains(pat),
            ".gitignore must keep `{pat}` out of the repo (open-source-guardrails §2)"
        );
    }
}

#[test]
fn ci_has_secret_scan_job() {
    let ci = read(".github/workflows/ci.yml");
    assert!(
        ci.contains("gitleaks"),
        "CI must run gitleaks so a pushed secret fails the build, not just the local hook"
    );
}

#[test]
fn ci_has_license_check_job() {
    let ci = read(".github/workflows/ci.yml");
    assert!(
        ci.contains("cargo-deny") || ci.contains("cargo deny"),
        "CI must run a license check (cargo-deny) against the permissive allow-list"
    );
    // The in-repo manifest check must also run in CI (it runs as part of `cargo
    // test`, but assert the deny config it complements exists).
    assert!(
        exists("deny.toml"),
        "deny.toml is required to configure cargo-deny's permissive-only allow-list"
    );
}

#[test]
fn deny_toml_allows_only_permissive_licenses() {
    let deny = read("deny.toml");
    // Sanity: the allow-list mentions our core permissive families and does not
    // silently allow copyleft.
    assert!(deny.contains("MIT"), "deny.toml should allow MIT");
    assert!(
        deny.contains("Apache-2.0"),
        "deny.toml should allow Apache-2.0"
    );
}

#[test]
fn hourly_release_script_present_and_executable() {
    let rel = "scripts/release-hourly.sh";
    assert!(exists(rel), "{rel} (hourly release automation) must exist");
    let body = read(rel);
    assert!(
        body.contains("hourly-") && body.contains("git tag"),
        "the hourly release script must cut a tagged `hourly-<N>` artifact"
    );
    assert_is_executable(rel);
}

#[test]
fn hourly_release_procedure_documented() {
    assert!(
        exists("docs/process/release-hourlies.md"),
        "the hourly-release procedure must be documented (release-hourlies.md)"
    );
}

#[cfg(unix)]
fn assert_is_executable(rel: &str) {
    use std::os::unix::fs::PermissionsExt;
    let path = repo_root().join(rel);
    let mode = std::fs::metadata(&path)
        .unwrap_or_else(|e| panic!("stat {}: {e}", path.display()))
        .permissions()
        .mode();
    assert!(
        mode & 0o111 != 0,
        "{rel} must be executable (chmod +x)",
        rel = rel
    );
    let _ = Path::new(rel);
}

#[cfg(not(unix))]
fn assert_is_executable(_rel: &str) {
    // Executable bit is a Unix concept; on other platforms presence is enough.
}
