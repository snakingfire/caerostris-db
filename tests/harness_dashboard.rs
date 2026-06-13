//! Board/pace dashboard generator checks (rubric Cat. 12, board item T-0004).
//!
//! `scripts/board/dashboard.sh` renders a read-only markdown snapshot of the
//! project's live state: item counts by status + by epic, a pace metric vs. T0,
//! the latest rubric overall score, and the current blockers. These tests build
//! a *fixture* board under a temp `CAERO_ROOT` (so they never depend on the live
//! board's moving state) and assert the generator's output is correct,
//! deterministic, side-effect-light, and fast.
//!
//! The script is the unit under test; Rust drives it so the whole thing runs
//! under `cargo nextest run` / `cargo test --workspace` like the rest of the
//! suite (cf. `tests/repo_hygiene.rs`).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn dashboard_script() -> PathBuf {
    repo_root().join("scripts/board/dashboard.sh")
}

/// A throwaway fixture tree with a `.project/board/tasks/` board, a pace ledger,
/// and an optional rubric report — rooted at a unique temp dir we clean up.
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
        root.push(format!("caero-dash-{tag}-{nanos}-{}", std::process::id()));
        std::fs::create_dir_all(root.join(".project/board/tasks")).unwrap();
        std::fs::create_dir_all(root.join(".project/reports")).unwrap();
        std::fs::create_dir_all(root.join(".project/pace")).unwrap();
        Self { root }
    }

    /// Write a minimal board item with the given frontmatter fields.
    fn task(&self, id: &str, status: &str, epic: &str, title: &str) {
        let body = format!(
            "---\nid: {id}\ntitle: {title}\ntype: task\nstatus: {status}\npriority: P1\nassignee:\nepic: {epic}\ndeps: []\nrubric_refs: [12]\nestimate: S\ncreated: T0\nupdated: T+1:00\n---\n\n## Context\nfixture\n"
        );
        std::fs::write(
            self.root.join(format!(".project/board/tasks/{id}.md")),
            body,
        )
        .unwrap();
    }

    /// Pin a T0 in the pace ledger so the pace metric is computable.
    fn pace(&self, t0_iso: &str) {
        let body = format!(
            "# Pace & Deadline Ledger\n\n- **T0 (autonomous run start):** `{t0_iso}` (fixture)\n- **BUILD_HOURS:** 4\n"
        );
        std::fs::write(self.root.join(".project/pace/deadline.md"), body).unwrap();
    }

    /// Drop a rubric report whose OVERALL row carries `score`.
    fn rubric_report(&self, name: &str, score: u32) {
        let body = format!(
            "# Rubric grade\n\n| Cat | Name | Weight | Score |\n|----:|------|------:|------:|\n| | **OVERALL** | **100** | **{score}** | |\n"
        );
        std::fs::write(self.root.join(format!(".project/reports/{name}")), body).unwrap();
    }

    /// Drop a rubric report whose OVERALL row carries an *approximate* score
    /// written `**~25**` — the live grader's actual format (it prefixes the
    /// running estimate with a tilde).
    fn rubric_report_approx(&self, name: &str, score: u32) {
        let body = format!(
            "## Headline: overall ~{score}\n\n| | **OVERALL** | 100 | **~{score}** | sum | |\n"
        );
        std::fs::write(self.root.join(format!(".project/reports/{name}")), body).unwrap();
    }

    /// Run the dashboard generator against this fixture; return (stdout, generated md).
    fn run(&self) -> (String, String) {
        let out = Command::new("bash")
            .arg(dashboard_script())
            .env("CAERO_ROOT", &self.root)
            .output()
            .expect("failed to spawn dashboard.sh");
        assert!(
            out.status.success(),
            "dashboard.sh exited non-zero: stdout={} stderr={}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let printed = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let path = Path::new(&printed);
        assert!(
            path.exists(),
            "dashboard.sh must print the path of the file it wrote; got {printed:?}"
        );
        let md = std::fs::read_to_string(path).unwrap();
        (printed, md)
    }

    /// How many dashboard-*.md files currently exist under reports/.
    fn dashboard_count(&self) -> usize {
        std::fs::read_dir(self.root.join(".project/reports"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.starts_with("dashboard-") && n.ends_with(".md"))
                    .unwrap_or(false)
            })
            .count()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn script_exists_and_is_executable() {
    let s = dashboard_script();
    assert!(s.exists(), "scripts/board/dashboard.sh must exist (T-0004)");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&s).unwrap().permissions().mode();
        assert!(
            mode & 0o111 != 0,
            "dashboard.sh must be executable (chmod +x)"
        );
    }
}

#[test]
fn counts_items_by_status() {
    let fx = Fixture::new("status");
    fx.pace("2026-06-13T18:24:00Z");
    fx.task("T-9001", "done", "EPIC-001", "alpha");
    fx.task("T-9002", "done", "EPIC-001", "beta");
    fx.task("T-9003", "ready", "EPIC-002", "gamma");
    fx.task("T-9004", "in_progress", "EPIC-002", "delta");
    fx.task("T-9005", "backlog", "EPIC-002", "epsilon");

    let (_path, md) = fx.run();

    // Status table must report the exact tallies for the fixture board.
    assert!(md.contains("| done | 2 |"), "expected 2 done; got:\n{md}");
    assert!(md.contains("| ready | 1 |"), "expected 1 ready; got:\n{md}");
    assert!(
        md.contains("| in_progress | 1 |"),
        "expected 1 in_progress; got:\n{md}"
    );
    assert!(
        md.contains("| backlog | 1 |"),
        "expected 1 backlog; got:\n{md}"
    );
    assert!(
        md.contains("**total**") && md.contains("**5**"),
        "expected total 5; got:\n{md}"
    );
}

#[test]
fn counts_items_by_epic() {
    let fx = Fixture::new("epic");
    fx.pace("2026-06-13T18:24:00Z");
    fx.task("T-9101", "done", "EPIC-001", "a");
    fx.task("T-9102", "ready", "EPIC-001", "b");
    fx.task("T-9103", "ready", "EPIC-002", "c");

    let (_path, md) = fx.run();
    assert!(
        md.contains("| EPIC-001 | 2 |"),
        "EPIC-001 should tally 2; got:\n{md}"
    );
    assert!(
        md.contains("| EPIC-002 | 1 |"),
        "EPIC-002 should tally 1; got:\n{md}"
    );
}

#[test]
fn reports_pace_metric_against_t0() {
    let fx = Fixture::new("pace");
    // T0 in the past so elapsed is positive and deterministic-ish (> 0 min).
    fx.pace("2020-01-01T00:00:00Z");
    fx.task("T-9201", "done", "EPIC-001", "a");
    fx.task("T-9202", "backlog", "EPIC-001", "b");

    let (_path, md) = fx.run();
    assert!(
        md.to_lowercase().contains("pace"),
        "dashboard must have a Pace section; got:\n{md}"
    );
    // 1 of 2 done, against a fixed past T0.
    assert!(
        md.contains("1 / 2") || md.contains("1/2") || md.contains("**1**"),
        "pace metric should reflect 1 of 2 done; got:\n{md}"
    );
    assert!(
        md.to_lowercase().contains("elapsed") || md.to_lowercase().contains("min"),
        "pace metric should report elapsed time; got:\n{md}"
    );
}

#[test]
fn surfaces_latest_rubric_score() {
    let fx = Fixture::new("rubric");
    fx.pace("2026-06-13T18:24:00Z");
    fx.task("T-9301", "done", "EPIC-001", "a");
    // Two reports; lexical-max name is the latest and must win.
    fx.rubric_report("rubric-T+01-00.md", 11);
    fx.rubric_report("rubric-T+02-00.md", 42);

    let (_path, md) = fx.run();
    assert!(
        md.contains("42"),
        "dashboard must surface the latest rubric overall score (42); got:\n{md}"
    );
    assert!(
        md.contains("rubric-T+02-00.md"),
        "dashboard must name the latest report it read; got:\n{md}"
    );
}

#[test]
fn surfaces_approximate_rubric_score() {
    // The live grader writes the OVERALL score as `**~25**` (tilde-prefixed
    // estimate). The dashboard must surface that, not fall back to "n/a".
    let fx = Fixture::new("rubric-approx");
    fx.pace("2026-06-13T18:24:00Z");
    fx.task("T-9351", "done", "EPIC-001", "a");
    fx.rubric_report_approx("rubric-T+03-02.md", 25);

    let (_path, md) = fx.run();
    assert!(
        md.contains("25") && !md.contains("| `rubric-T+03-02.md` | n/a |"),
        "dashboard must parse the tilde-prefixed approximate OVERALL score; got:\n{md}"
    );
}

#[test]
fn lists_blockers() {
    let fx = Fixture::new("blockers");
    fx.pace("2026-06-13T18:24:00Z");
    fx.task("T-9401", "blocked", "EPIC-001", "stuck-on-merge");
    fx.task("T-9402", "ready", "EPIC-001", "fine");

    let (_path, md) = fx.run();
    assert!(
        md.contains("T-9401") && md.contains("stuck-on-merge"),
        "dashboard must list blocked items; got:\n{md}"
    );
    // A non-blocked item must NOT appear in the blockers table row.
    assert!(
        !md.contains("| T-9402 | fine |"),
        "non-blocked items must not appear as blockers; got:\n{md}"
    );
}

#[test]
fn no_blockers_renders_a_clear_none() {
    let fx = Fixture::new("noblock");
    fx.pace("2026-06-13T18:24:00Z");
    fx.task("T-9501", "ready", "EPIC-001", "a");

    let (_path, md) = fx.run();
    assert!(
        md.to_lowercase().contains("none"),
        "with no blockers the dashboard must say so explicitly; got:\n{md}"
    );
}

#[test]
fn repeatable_without_extra_side_effects() {
    // Each run writes exactly one new dashboard file and touches nothing else.
    let fx = Fixture::new("idempotent");
    fx.pace("2026-06-13T18:24:00Z");
    fx.task("T-9601", "done", "EPIC-001", "a");

    assert_eq!(fx.dashboard_count(), 0);
    let (_p1, _m1) = fx.run();
    let after_first = fx.dashboard_count();
    assert!(
        after_first >= 1,
        "first run must write a dashboard file (got {after_first})"
    );

    // Board files must be untouched by the generator (read-only on the board).
    let board_before =
        std::fs::read_to_string(fx.root.join(".project/board/tasks/T-9601.md")).unwrap();
    let (_p2, _m2) = fx.run();
    let board_after =
        std::fs::read_to_string(fx.root.join(".project/board/tasks/T-9601.md")).unwrap();
    assert_eq!(
        board_before, board_after,
        "dashboard.sh must never mutate board files"
    );
}

#[test]
fn runs_well_under_five_seconds() {
    let fx = Fixture::new("fast");
    fx.pace("2026-06-13T18:24:00Z");
    for i in 0..60 {
        fx.task(&format!("T-97{i:02}"), "backlog", "EPIC-001", "bulk");
    }
    let start = Instant::now();
    let (_p, _m) = fx.run();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 5,
        "dashboard generation must run in under 5s; took {elapsed:?}"
    );
}
