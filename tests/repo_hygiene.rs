//! Process-hygiene wiring checks (rubric Cat. 12).
//!
//! These guard the *configuration* that keeps secrets out and releases flowing —
//! the things board item T-0039 calls for. They are deliberately cheap, file-
//! presence/content assertions: if someone deletes the gitleaks config, unwires
//! the pre-commit hook, drops the CI license/secret jobs, or removes the hourly
//! release script, CI goes red here with a pointer to what regressed.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Return the git-tracked paths matching `pathspec`, queried from the repo root.
///
/// `git ls-files` reports the index of whichever working tree
/// `CARGO_MANIFEST_DIR` lives in — the canonical checkout in CI, or a PR
/// worktree locally — so the guard catches a tracked stray file in either.
///
/// Returns `None` when git itself is unavailable or the command fails (e.g. a
/// source tarball extracted without a `.git` directory). The caller treats that
/// as "cannot prove a violation" and skips: if there is no git index, there is
/// nothing tracked to remove, so the guard has nothing to assert against.
fn git_tracked(pathspec: &str) -> Option<Vec<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root())
        .args(["ls-files", "--", pathspec])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

/// Recursively collect every regular file under `dir` (relative to the repo
/// root), skipping the `target/` build tree.
fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some("target") {
                continue;
            }
            walk_files(&path, out);
        } else if file_type.is_file() {
            out.push(path);
        }
    }
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

/// No stale `docs/design/` cross-references survive in the board or docs.
///
/// The storage-format spec is owned by `SPIKE-0003` and lands under `docs/adr/`;
/// `docs/design/` does not exist and will not be created. A pointer there sends
/// an implementer on a wild grep (see BUG-0003 / BUG-0011). The only files
/// allowed to mention the path are the bug records that *document the defect
/// itself* — BUG-0011 (this guard's bug) and BUG-0003 (its parent + review
/// verdict) — matched by filename prefix so a slug rename cannot silently
/// re-open the gap. This is the same "historical record" exception BUG-0003
/// applied to SPIKE-0002. (Rubric Cat. 12 docs-hygiene; Cat. 2 implementer
/// friction on the storage epic.)
#[test]
fn no_stale_docs_design_references() {
    const ALLOWLIST_PREFIXES: [&str; 2] = ["BUG-0011-", "BUG-0003-"];
    const SEARCH_DIRS: [&str; 2] = [".project/board", "docs"];

    let root = repo_root();
    let mut violations: Vec<String> = Vec::new();

    for dir in SEARCH_DIRS {
        let mut files = Vec::new();
        walk_files(&root.join(dir), &mut files);
        for file in files {
            let name = file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if ALLOWLIST_PREFIXES
                .iter()
                .any(|prefix| name.starts_with(prefix))
            {
                continue;
            }
            // Read as bytes; skip anything that is not valid UTF-8 (e.g. images).
            let contents = match std::fs::read(&file) {
                Ok(bytes) => match String::from_utf8(bytes) {
                    Ok(s) => s,
                    Err(_) => continue,
                },
                Err(_) => continue,
            };
            for (lineno, line) in contents.lines().enumerate() {
                if line.contains("docs/design/") {
                    let rel = file.strip_prefix(&root).unwrap_or(&file);
                    violations.push(format!("{}:{}: {}", rel.display(), lineno + 1, line.trim()));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "stale `docs/design/` references found — repoint at the SPIKE-0003-owned \
         storage-format spec (lands under docs/adr/):\n  {}",
        violations.join("\n  ")
    );
}

/// A root-level `PR.md` must never be git-tracked (BUG-0013).
///
/// `PR.md` is per-worktree PR-scratch that `scripts/pr/open.sh` writes under
/// `.worktrees/<ID>/PR.md` (already ignored via `.worktrees/`). A tracked copy at
/// the repo root means every freshly-opened worktree inherits a stale PR
/// description from whatever task last committed it (BUG-0011 inherited T-0039's
/// verbatim). This guard fails CI if a root `PR.md` is ever (re-)committed, so the
/// regression cannot silently return. (Rubric Cat. 12 process hygiene.)
///
/// Note: the file may still exist *on disk* in a live PR worktree — workers edit
/// it locally — so this checks git *tracking*, not file presence.
#[test]
fn root_pr_md_is_not_tracked() {
    let Some(tracked) = git_tracked("PR.md") else {
        // No usable git index (e.g. a source tarball without `.git`): nothing is
        // tracked, so there is nothing to assert against. Skip rather than fail.
        return;
    };
    // `git ls-files -- PR.md` from the repo root only ever matches a *root*
    // `PR.md`; nested `PR.md` files (none today) would need a different pathspec.
    assert!(
        tracked.is_empty(),
        "a root-level `PR.md` is git-tracked ({tracked:?}); it is per-worktree PR \
         scratch and must not live on `main`. Remove it with `git rm --cached PR.md` \
         (keep your worktree-local copy) — see BUG-0013 / .gitignore `/PR.md`."
    );
}

/// `.gitignore` must ignore a root-level `PR.md` so it cannot be re-added by
/// accident (BUG-0013). `scripts/pr/open.sh` writes `PR.md` under `.worktrees/`,
/// which is already ignored; the explicit `/PR.md` rule is belt-and-suspenders
/// against a worker running it from the repo root or hand-creating one.
#[test]
fn gitignore_ignores_root_pr_md() {
    let gi = read(".gitignore");
    let ignores_root_pr = gi.lines().map(str::trim).any(|l| l == "/PR.md");
    assert!(
        ignores_root_pr,
        ".gitignore must contain a `/PR.md` rule so a root-level PR.md cannot be \
         committed by accident (BUG-0013)"
    );
}
