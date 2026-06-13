---
id: EPIC-010
title: Harden the autonomous harness (self-expansion & epoch recycling)
type: epic
status: backlog
priority: P0
assignee:
epic:
deps: []
rubric_refs: [12]
estimate: M
created: T0
updated: T0
---

## Context

caerostris-db is built by an autonomous agent swarm operating under a 4-hour wallclock deadline. Engineering and process health (Cat. 12, weight 4) requires: a live work board reflecting reality, ADRs for every major decision, ≥1 hourly release per hour, CLAUDE.md and agent memory kept current, gitleaks clean, and all deps + datasets license-verified.

This epic covers the meta-infrastructure that keeps the swarm productive and self-sustaining across the full run: (1) **epoch recycling** — when a run approaches its per-run agent cap, the mainspring must cleanly hand off state so the next epoch resumes at full throughput rather than restarting from scratch; (2) a **board/pace dashboard** — a lightweight report (regenerated frequently) showing status counts, pace vs. deadline, and rubric score trend; (3) **STOP-sentinel handling** — clean shutdown when a sentinel is detected, preserving board/ADR/commit state; (4) the **planner-decomposer standing task** (T-0003) as a continuously-running role that decomposes epics into ready stories and tasks; and (5) ongoing **process hygiene automation** — commit prefix enforcement, gitleaks in pre-commit, license-check on new deps, memory/CLAUDE.md update reminders.

Relevant requirements: R12 (hourly releases, board hygiene, ADRs, CI green, gitleaks, license-clean).

## Acceptance criteria

- [ ] Epoch-recycling mechanism documented and implemented: at epoch boundary, the mainspring serialises in-flight context (open tasks, pending decisions, current rubric score) and relaunches the swarm; throughput does not drop to zero between epochs.
- [ ] Board/pace dashboard: a script or CI step generates `.project/reports/dashboard-<timestamp>.md` with status counts per state, items per epic, pace (items done / elapsed vs. projected), and latest rubric scores.
- [ ] STOP-sentinel handling: when the sentinel is detected, in-flight agents reach a clean checkpoint (commit board state, push ADRs, no partial commits in git), then exit gracefully.
- [ ] Planner-decomposer operational: T-0003 is executed and every epic has ≥3 child stories/tasks on the board with accurate deps and rubric_refs.
- [ ] ≥1 hourly release cut per hour of the run (tagged commit or release artifact) throughout the 4-hour window.
- [ ] gitleaks pre-commit hook configured and passing; no secrets in any commit.
- [ ] License check: every new dependency added during the run is recorded in a `docs/licenses/` manifest entry with its SPDX identifier and compatibility assessment.
- [ ] `./format_code.sh` green continuously; CI stays green.

## Notes / log

T-0004 (epoch recycling + dashboard) and T-0003 (planner decompose all epics) are the immediate kickoff tasks. Both are `ready` from T0.
