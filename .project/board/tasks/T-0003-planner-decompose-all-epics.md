---
id: T-0003
title: Planner-decomposer standing task — decompose every epic into ready stories/tasks
type: task
status: done
priority: P0
assignee: planner-decomposer
epic: EPIC-010
deps: []
rubric_refs: [12]
estimate: M
created: T0
updated: T0+0:20
---

## Context

The 10 seeded epics are large and intentionally coarse. For the agent swarm to make parallel progress each epic must be decomposed into concrete, independently claimable stories and tasks with accurate `deps`, `rubric_refs`, and `estimate` values. Without this decomposition, agents have no `ready` items to claim — the entire board stalls.

This is the **planner-decomposer's standing first task**: before any significant implementation begins, every epic on the board must have ≥3 child stories/tasks created as `.md` files in `.project/board/tasks/`, each following the template schema exactly.

**Decomposition rules:**
- Each child item must have its parent epic in the `epic` field.
- Each child item must have explicit `deps` (using IDs of other board items or empty `[]`).
- Design-before-code ordering must be enforced: implementation tasks that depend on an unratified design (SPIKE) must list that SPIKE in their `deps` and start as `status: backlog`, not `ready`.
- Items that have no blocking deps and are ready to start immediately get `status: ready`.
- Prefer `estimate: S` or `M`; split anything `L` into smaller children.
- `rubric_refs` must be accurate — each task advances specific rubric categories.
- Acceptance criteria must be concrete and testable (not vague).

**Epics to decompose (all 10):**
- EPIC-001 (storage format + commit)
- EPIC-002 (openCypher engine + TCK)
- EPIC-003 (latency envelope)
- EPIC-004 (ACID txn + formal verification)
- EPIC-005 (secondary indices)
- EPIC-006 (concurrency + attach modes)
- EPIC-007 (Python bindings)
- EPIC-008 (caching)
- EPIC-009 (testing + coverage)
- EPIC-010 (harness hardening)

Already created: SPIKE-0001, SPIKE-0002, SPIKE-0003, T-0001, T-0002, T-0004, T-0005 (these are the kickoff items; each epic needs additional children beyond these).

## Acceptance criteria

- [ ] Each of the 10 epics has ≥3 child stories/tasks on the board (files in `.project/board/tasks/`) with correct `epic`, `deps`, `rubric_refs`, and `status` fields.
- [ ] Design-before-code ordering respected: implementation tasks under EPIC-001, EPIC-003, EPIC-004 that depend on unratified spikes (SPIKE-0001, SPIKE-0002, SPIKE-0003) are `status: backlog` with those spikes in `deps`.
- [ ] No task is created with `status: ready` if it has a `dep` that is not `done` — the planner-decomposer checks this before writing each file.
- [ ] Every new task file passes the template schema: has all required frontmatter fields, a non-empty Context section, and ≥2 concrete acceptance-criteria bullets.
- [ ] After decomposition, `rg 'status: ready' .project/board/tasks/` returns ≥10 items (enough for the swarm to fan out immediately).
- [ ] A brief decomposition log committed to `.project/decisions/` noting any significant architectural assumptions made during decomposition.
- [ ] `./format_code.sh` green (no Rust changes in this task, but the task itself must be clean YAML/Markdown).

## Notes / log

This task is marked `done` only when every epic has ≥3 children and the ready queue has ≥10 items. The planner-decomposer role should continue to decompose new epics as they are added by the rubric grader or steering committee throughout the run.

- **T0+0:20 (planner-decomposer):** DONE. All 10 epics decomposed. Filed
  T-0006…T-0040, STORY-001, SPIKE-0009 (33 new items; board now 65 files).
  Children/epic: 001=7, 002=9, 003=8, 004=8, 005=4, 006=5, 007=3, 008=3, 009=5,
  010=3 (all ≥3). Ready queue = 17 (≥10). Design-before-code enforced: storage
  impl gated on SPIKE-0003; commit/ACID impl on SPIKE-0002/0005; latency impl on
  SPIKE-0001/0004/0006/0007 — all such tasks `backlog` with the spike in `deps`.
  Corrected inherited honesty violations: T-0001 (dep T-0000) and SPIKE-0004 (dep
  SPIKE-0001) flipped ready→backlog so no `ready` task has a non-`done` dep.
  Decomposition rationale + assumptions in
  `.project/decisions/0011-board-decomposition-of-all-epics.md`. Bootstrap sentinel
  `.project/.bootstrapped` written.
