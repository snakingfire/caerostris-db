---
id: T-0004
title: Mainspring epoch-recycling, board/pace dashboard, and STOP-sentinel handling
type: task
status: ready
priority: P0
assignee:
epic: EPIC-010
deps: []
rubric_refs: [12]
estimate: M
created: T0
updated: T0
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
