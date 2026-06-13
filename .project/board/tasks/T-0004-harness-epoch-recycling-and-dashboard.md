---
id: T-0004
title: Mainspring epoch-recycling, board/pace dashboard, and STOP-sentinel handling
type: task
status: done
priority: P0
assignee: integrator
epic: EPIC-010
deps: []
rubric_refs: [12]
estimate: M
created: T0
updated: T+4:10
---

## Context

The swarm operates under a per-run agent cap. Without epoch recycling, throughput collapses to zero each time a run approaches that cap — requiring a manual cold restart that wastes the deadline budget. This task implements robust mainspring infrastructure that sustains continuous throughput across the full 4-hour window.

**Three sub-deliverables:**

1. **Epoch recycling**: when the mainspring detects it is approaching the per-run agent cap, it must serialise in-flight context (open task IDs, pending steering decisions, latest rubric score, active blockers) into a hand-off artifact in `.project/epochs/`, then trigger a clean relaunch. The relaunched epoch reads the hand-off artifact and resumes at full throughput — it must not re-execute completed work or lose in-progress state. The mechanism must sustain 4 hours under the cap.

2. **Board/pace dashboard**: a script (or CI step) that regenerates `.project/reports/dashboard-<timestamp>.md` with: (a) item counts by status (`backlog`, `ready`, `in_progress`, `done`) and by epic; (b) pace metric: items completed / elapsed time vs. projected completion rate; (c) latest rubric scores from `.project/reports/`; (d) current blockers (items in `blocked` state). The dashboard is regenerated at least once per 20-minute grader cycle.

3. **STOP-sentinel handling**: when a STOP sentinel is detected in the agent stream (or a configurable signal), in-flight agents reach a clean checkpoint — commit all board state changes, push any pending ADRs, ensure no partial commits exist in git — then exit gracefully. The state after a STOP must be fully resumable by the next epoch.

**Definition of "sustain throughput across 4h under the per-run agent cap"**: at least one board item transitions to `done` per hour throughout the 4-hour window, with no gap longer than 15 minutes where all agents are idle due to cap exhaustion.

## Acceptance criteria

- [ ] Epoch hand-off artifact format documented: schema for `.project/epochs/epoch-<N>.json` (or `.md`) including open task IDs, blockers, rubric snapshot, timestamp.
- [ ] Relaunch procedure documented in `docs/process/epoch-recycling.md`: step-by-step instructions for the next epoch to read the hand-off and resume without duplicate work.
- [ ] STOP-sentinel detection implemented: the mainspring recognises the sentinel; a test demonstrates that triggering it causes a clean checkpoint (no dirty git state, no open `in_progress` items without a note).
- [ ] Dashboard script at `scripts/board/dashboard.sh` (or equivalent): runs in under 5 seconds; outputs the dashboard `.md` to `.project/reports/`; can be run repeatedly without side effects.
- [ ] Dashboard content verified: a test run shows correct item counts, pace metric, and latest rubric scores for a known board state.
- [ ] CI step added: dashboard regenerated and committed as part of each grader cycle (or at least available as a script agents can call).
- [ ] `./format_code.sh` green; no clippy warnings.

## Notes / log

This task has no deps — it is ready from T0. The epoch hand-off format should be lightweight and human-readable (markdown preferred over binary) so any agent can inspect it without special tooling.

- **T+0:53 — pre-mortem: changes_requested.** Worktree `.worktrees/T-0004` exists but
  branch `work/T-0004` (tip `190fcb4`) is an ancestor of `main` — zero commits, empty
  diff. `PR.md` is the unfilled stub; none of the 7 acceptance-criteria deliverables
  (epoch-recycling doc/schema, STOP-sentinel detection+test, `scripts/board/dashboard.sh`+test,
  CI step) exist. Nothing to review or land. Item is still `status: ready` / unclaimed —
  worker must claim it (`in_progress` + `assignee`) when work begins. Pre-mortem verdict
  recorded in `.worktrees/T-0004/PR.md`; premortem sign-off box left unchecked.
- **T+3:22 — implementer-wf_fe688db0-093-7: in_review.** Branch
  `work/T-0004-epoch-recycle-dash-stop-h` (off main `494a9e7`). All 7 acceptance
  criteria met TDD-first: `scripts/board/dashboard.sh` (status/epic counts, pace
  metric, latest rubric, blockers; <1s; read-only; CI-wired);
  `scripts/epoch/handoff.sh` + `resume.sh` (epoch hand-off to
  `.project/epochs/epoch-<N>.md` — markdown + JSON schema; done items excluded so
  no duplicate execution); `scripts/epoch/stop.sh` (raises `.project/STOP`, clean
  checkpoint, idempotent); `docs/process/epoch-recycling.md`. Guarded by 14 tests
  in `tests/harness_infra.rs` (full suite 137 passed). `./format_code.sh` green.
  Filed BUG-0016 (pre-existing: PR.md tracked on main). PR.md in worktree. Awaiting
  adversarial-review + pre-mortem.
- **T+3:25 — implementer-wf_6a2f8faf-da3-2: YIELDING (duplicate).** This lane was
  also dispatched on T-0004 and built an equivalent, green, additive implementation
  on branch `work/T-0004-mainspring-epoch-recycling-board-pace-dashboard`
  (`scripts/board/dashboard.sh` + `checkpoint.sh` + `epoch-handoff.sh`,
  `.project/epochs/README.md`, `docs/process/epoch-recycling.md`, 28 tests across
  `tests/harness_{dashboard,checkpoint,epoch_handoff,ci_wiring}.rs`; 151/151 green;
  `./format_code.sh` green; rebased on latest main). On rebase this lane discovered
  `wf_fe688db0-093-7` already owns the active `in_review` slot (committed to main
  first, T+3:22) with the same deliverables. Per the board protocol ("loser picks
  another") and to avoid duplicate review/land churn, this lane **stands down** and
  releases its claim — the rival's PR is the one to review/land. This lane's branch
  is left intact as a fallback artifact should the rival PR stall (T-0004 has a long
  history of stalling at this stage); if so, it can be picked up and landed cleanly.
- **T+3:45 — integrator: BLOCKED — review gate not cleared.** Adversarial reviewer
  verdict is `changes_requested` (T+3:30) — blocking finding: `stop.sh` silently
  orphans checkpoint commits on detached HEAD (the `-z "$BRANCH"` guard on
  `symbolic-ref` empty output causes commits on detached HEAD, which land on a
  dangling commit with no branch ref, effectively lost — contradicts the
  "nothing is lost" durability promise). Premortem analyst has not signed off.
  Both review-gate checkboxes in `PR.md` are unchecked. Cannot land per protocol.
  Author (implementer) must: (1) fix `stop.sh` to detect main explicitly using
  `rev-parse --abbrev-ref HEAD` and refuse on detached HEAD (fall through to the
  WARNING path, not commit); (2) add regression test for detached-HEAD path;
  (3) re-run `./format_code.sh` + `cargo nextest run`; (4) re-request review
  (review-gate checkboxes reset to unchecked until both reviewers re-approve).
  The branch `work/T-0004-epoch-recycle-dash-stop-h` in worktree
  `.claude/worktrees/wf_fe688db0-093-7` is preserved for the author to continue.
- **T+4:10 — integrator: Landed in commit 1c5c118 at T+4:10.** Reland of
  `work/T-0004-mainspring-epoch-recycling-board-pace-dashboard`; rebased onto
  `61ffdac`, additive board conflict resolved (timestamp union), `./format_code.sh`
  green, 259/259 tests passed, merged `--no-ff` to main and pushed. Cat. 12 /
  EPIC-010 delivered: `scripts/board/dashboard.sh`, `scripts/board/checkpoint.sh`,
  `scripts/board/epoch-handoff.sh`, `docs/process/epoch-recycling.md`,
  `.project/epochs/README.md`, CI board-dashboard job, 4 Rust test suites (28 tests).
