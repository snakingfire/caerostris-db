//! STOP-sentinel clean-checkpoint checks (rubric Cat. 12, board item T-0004).
//!
//! `scripts/board/checkpoint.sh` is the graceful-shutdown verifier the swarm
//! runs when a STOP sentinel (`.project/STOP`) appears: it confirms the tree is
//! in a *resumable* state — no dirty/partial git state, and no `in_progress`
//! board item without a note in its log (so the next epoch can pick up exactly
//! where this one left off). It exits 0 when the checkpoint is clean and non-zero
//! (with a diagnostic) otherwise, so a relaunch can gate on it.
//!
//! These tests build a fixture git repo + board under a temp `CAERO_ROOT` and
//! drive the script through the clean / dirty / STOP-present cases. As with the
//! dashboard tests, the script is the unit under test and Rust drives it so it
//! runs under the normal `cargo` suite.

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn checkpoint_script() -> PathBuf {
    repo_root().join("scripts/board/checkpoint.sh")
}

/// A throwaway *git* fixture tree with a board, so we can stage dirty state.
struct GitFixture {
    root: PathBuf,
}

impl GitFixture {
    fn new(tag: &str) -> Self {
        let mut root = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        root.push(format!("caero-ckpt-{tag}-{nanos}-{}", std::process::id()));
        std::fs::create_dir_all(root.join(".project/board/tasks")).unwrap();
        let fx = Self { root };
        fx.git(&["init", "-q"]);
        fx.git(&["config", "user.email", "test@example.com"]);
        fx.git(&["config", "user.name", "ckpt-test"]);
        fx
    }

    fn git(&self, args: &[&str]) -> std::process::Output {
        Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .expect("git spawn")
    }

    fn commit_all(&self, msg: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", msg]);
    }

    /// Write a board item; `note` (if Some) is appended under a Notes/log section.
    fn task(&self, id: &str, status: &str, note: Option<&str>) {
        let mut body = format!(
            "---\nid: {id}\ntitle: fixture {id}\ntype: task\nstatus: {status}\npriority: P1\nassignee: lane-x\nepic: EPIC-001\ndeps: []\nrubric_refs: [12]\nestimate: S\ncreated: T0\nupdated: T+1:00\n---\n\n## Context\nfixture\n\n## Notes / log\n"
        );
        if let Some(n) = note {
            body.push_str(&format!("- {n}\n"));
        }
        std::fs::write(
            self.root.join(format!(".project/board/tasks/{id}.md")),
            body,
        )
        .unwrap();
    }

    fn write_stop(&self) {
        std::fs::write(self.root.join(".project/STOP"), "STOP requested by test\n").unwrap();
    }

    /// Run checkpoint.sh; return (exit_code, stdout, stderr).
    fn run(&self) -> (i32, String, String) {
        let out = Command::new("bash")
            .arg(checkpoint_script())
            .env("CAERO_ROOT", &self.root)
            .current_dir(&self.root)
            .output()
            .expect("failed to spawn checkpoint.sh");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    }
}

impl Drop for GitFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn script_exists_and_is_executable() {
    let s = checkpoint_script();
    assert!(
        s.exists(),
        "scripts/board/checkpoint.sh must exist (T-0004)"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&s).unwrap().permissions().mode();
        assert!(
            mode & 0o111 != 0,
            "checkpoint.sh must be executable (chmod +x)"
        );
    }
}

#[test]
fn clean_tree_with_noted_in_progress_passes() {
    let fx = GitFixture::new("clean");
    fx.task("T-1001", "done", None);
    // An in_progress item is fine *iff* it carries a note for the next epoch.
    fx.task(
        "T-1002",
        "in_progress",
        Some("T+1:00 handed off: parser half-done"),
    );
    fx.commit_all("fixture board");

    let (code, out, err) = fx.run();
    assert_eq!(
        code, 0,
        "a clean tree with a noted in_progress item is a valid checkpoint; out={out} err={err}"
    );
}

#[test]
fn dirty_git_state_fails_checkpoint() {
    let fx = GitFixture::new("dirty");
    fx.task("T-1101", "done", None);
    fx.commit_all("fixture board");
    // Introduce an UNCOMMITTED change → partial state → not resumable.
    std::fs::write(fx.root.join("uncommitted.txt"), "partial work\n").unwrap();

    let (code, out, err) = fx.run();
    assert_ne!(
        code, 0,
        "dirty/uncommitted git state must fail the checkpoint; out={out} err={err}"
    );
    let combined = format!("{out}{err}").to_lowercase();
    assert!(
        combined.contains("dirty")
            || combined.contains("uncommitted")
            || combined.contains("clean"),
        "failure must explain the dirty git state; got out={out} err={err}"
    );
}

#[test]
fn unnoted_in_progress_fails_checkpoint() {
    let fx = GitFixture::new("unnoted");
    fx.task("T-1201", "done", None);
    // in_progress with NO note → the next epoch can't know what was in flight.
    fx.task("T-1202", "in_progress", None);
    fx.commit_all("fixture board");

    let (code, out, err) = fx.run();
    assert_ne!(
        code, 0,
        "an in_progress item without a note must fail the checkpoint; out={out} err={err}"
    );
    let combined = format!("{out}{err}");
    assert!(
        combined.contains("T-1202"),
        "failure must name the un-noted in_progress item; got out={out} err={err}"
    );
}

#[test]
fn detects_stop_sentinel() {
    let fx = GitFixture::new("stop");
    fx.task("T-1301", "done", None);
    fx.commit_all("fixture board");
    fx.write_stop();
    // STOP is committed alongside the board so the tree stays clean.
    fx.commit_all("STOP sentinel");

    let (code, out, _err) = fx.run();
    // With a clean tree the checkpoint still passes, but it must REPORT that the
    // STOP sentinel is present so the operator/relaunch sees the standing-down.
    assert_eq!(
        code, 0,
        "clean tree + STOP is a valid (final) checkpoint; out={out}"
    );
    assert!(
        out.to_uppercase().contains("STOP"),
        "checkpoint must report the STOP sentinel when present; got:\n{out}"
    );
}

#[test]
fn no_stop_sentinel_is_reported_as_running() {
    let fx = GitFixture::new("nostop");
    fx.task("T-1401", "done", None);
    fx.commit_all("fixture board");

    let (code, out, _err) = fx.run();
    assert_eq!(
        code, 0,
        "clean running tree is a valid checkpoint; out={out}"
    );
    assert!(
        out.to_lowercase().contains("no stop") || out.to_lowercase().contains("running"),
        "checkpoint should note the absence of a STOP sentinel; got:\n{out}"
    );
}
