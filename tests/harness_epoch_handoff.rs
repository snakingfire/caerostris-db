//! Epoch hand-off artifact checks (rubric Cat. 12, board item T-0004).
//!
//! `scripts/board/epoch-handoff.sh` serialises the in-flight context an epoch
//! needs to hand to its successor when it recycles near the per-run agent cap:
//! the open task IDs (ready / in_progress / in_review / blocked), the current
//! blockers, the latest rubric snapshot, and a timestamp. The artifact is a
//! lightweight, human-readable markdown file under `.project/epochs/epoch-<N>.md`
//! (markdown over binary so any agent can inspect it without tooling — per the
//! task's Notes). The relaunched epoch reads it to resume without re-doing
//! completed work; the format is documented in `docs/process/epoch-recycling.md`.
//!
//! As with the other harness tests, the script is the unit under test and Rust
//! drives it over a fixture board under a temp `CAERO_ROOT`.

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn handoff_script() -> PathBuf {
    repo_root().join("scripts/board/epoch-handoff.sh")
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let mut root = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        root.push(format!("caero-epoch-{tag}-{nanos}-{}", std::process::id()));
        std::fs::create_dir_all(root.join(".project/board/tasks")).unwrap();
        std::fs::create_dir_all(root.join(".project/reports")).unwrap();
        std::fs::create_dir_all(root.join(".project/pace")).unwrap();
        let pace = "# Pace\n- **T0 (autonomous run start):** `2026-06-13T18:24:00Z`\n";
        std::fs::write(root.join(".project/pace/deadline.md"), pace).unwrap();
        Self { root }
    }

    fn task(&self, id: &str, status: &str, title: &str) {
        let body = format!(
            "---\nid: {id}\ntitle: {title}\ntype: task\nstatus: {status}\npriority: P1\nassignee:\nepic: EPIC-001\ndeps: []\nrubric_refs: [12]\nestimate: S\ncreated: T0\nupdated: T+1:00\n---\n\n## Context\nfixture\n"
        );
        std::fs::write(
            self.root.join(format!(".project/board/tasks/{id}.md")),
            body,
        )
        .unwrap();
    }

    fn rubric_report(&self, name: &str, score: u32) {
        let body = format!("# grade\n\n| | **OVERALL** | 100 | **~{score}** | sum | |\n");
        std::fs::write(self.root.join(format!(".project/reports/{name}")), body).unwrap();
    }

    /// Run with an explicit epoch number; return (stdout path, artifact body).
    fn run(&self, epoch: u32) -> (String, String) {
        let out = Command::new("bash")
            .arg(handoff_script())
            .arg(epoch.to_string())
            .env("CAERO_ROOT", &self.root)
            .output()
            .expect("failed to spawn epoch-handoff.sh");
        assert!(
            out.status.success(),
            "epoch-handoff.sh exited non-zero: stdout={} stderr={}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let printed = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let body = std::fs::read_to_string(&printed)
            .unwrap_or_else(|e| panic!("artifact not readable at {printed:?}: {e}"));
        (printed, body)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn script_exists_and_is_executable() {
    let s = handoff_script();
    assert!(
        s.exists(),
        "scripts/board/epoch-handoff.sh must exist (T-0004)"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&s).unwrap().permissions().mode();
        assert!(
            mode & 0o111 != 0,
            "epoch-handoff.sh must be executable (chmod +x)"
        );
    }
}

#[test]
fn writes_artifact_to_epochs_dir_named_by_number() {
    let fx = Fixture::new("path");
    fx.task("T-2001", "ready", "a");
    let (path, _body) = fx.run(7);
    assert!(
        path.contains(".project/epochs/epoch-7"),
        "artifact must live at .project/epochs/epoch-<N>; got {path}"
    );
}

#[test]
fn artifact_carries_open_task_ids() {
    let fx = Fixture::new("openids");
    fx.task("T-2101", "done", "completed-should-not-carry");
    fx.task("T-2102", "ready", "ready-work");
    fx.task("T-2103", "in_progress", "in-flight-work");
    fx.task("T-2104", "in_review", "awaiting-land");

    let (_path, body) = fx.run(1);
    // Open (resumable) work must be listed so the next epoch re-pulls it.
    assert!(
        body.contains("T-2102"),
        "ready item must be carried; got:\n{body}"
    );
    assert!(
        body.contains("T-2103"),
        "in_progress item must be carried; got:\n{body}"
    );
    assert!(
        body.contains("T-2104"),
        "in_review item must be carried; got:\n{body}"
    );
    // Completed work must NOT be in the open set (no re-execution).
    assert!(
        !body.contains("T-2101"),
        "done work must NOT be carried as open (would re-execute); got:\n{body}"
    );
}

#[test]
fn artifact_carries_blockers() {
    let fx = Fixture::new("blockers");
    fx.task("T-2201", "blocked", "stuck-item");
    fx.task("T-2202", "ready", "fine");

    let (_path, body) = fx.run(2);
    let lower = body.to_lowercase();
    assert!(
        lower.contains("blocker"),
        "artifact must have a blockers section; got:\n{body}"
    );
    assert!(
        body.contains("T-2201"),
        "blocked item must be carried as a blocker; got:\n{body}"
    );
}

#[test]
fn artifact_carries_rubric_snapshot() {
    let fx = Fixture::new("rubric");
    fx.task("T-2301", "ready", "a");
    fx.rubric_report("rubric-T+02-00.md", 42);

    let (_path, body) = fx.run(3);
    assert!(
        body.contains("42"),
        "artifact must snapshot the latest rubric score; got:\n{body}"
    );
    assert!(
        body.contains("rubric-T+02-00.md"),
        "artifact must name the report it snapshotted; got:\n{body}"
    );
}

#[test]
fn artifact_carries_timestamp_and_epoch_number() {
    let fx = Fixture::new("stamp");
    fx.task("T-2401", "ready", "a");
    let (_path, body) = fx.run(9);
    // ISO-8601 UTC timestamp (YYYY-MM-DDThh:mm:ssZ).
    assert!(
        body.contains("Z") && body.contains("-") && body.contains(":"),
        "artifact must carry a UTC timestamp; got:\n{body}"
    );
    assert!(
        body.contains("9"),
        "artifact must record its epoch number; got:\n{body}"
    );
}

#[test]
fn relaunch_procedure_is_documented() {
    let doc = repo_root().join("docs/process/epoch-recycling.md");
    assert!(
        doc.exists(),
        "docs/process/epoch-recycling.md must document the relaunch procedure"
    );
    let body = std::fs::read_to_string(&doc).unwrap().to_lowercase();
    for needle in ["epoch", "stop", "resume", "hand-off"] {
        assert!(
            body.contains(needle),
            "epoch-recycling.md must cover '{needle}'"
        );
    }
}

#[test]
fn epochs_dir_has_a_schema_readme() {
    let readme = repo_root().join(".project/epochs/README.md");
    assert!(
        readme.exists(),
        ".project/epochs/README.md must document the hand-off artifact schema"
    );
    let body = std::fs::read_to_string(&readme).unwrap().to_lowercase();
    for needle in ["open", "blocker", "rubric", "timestamp"] {
        assert!(
            body.contains(needle),
            "epochs/README.md schema must mention '{needle}'"
        );
    }
}
