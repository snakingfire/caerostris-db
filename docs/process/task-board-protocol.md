# Task Board Protocol — caerostris-db

> A file-based, git-native, conflict-minimizing "Linear board". State lives in
> [`../../.project/board/`](../../.project/board/). One file per work item so
> parallel agents editing *different* items never conflict.

## Layout

```
.project/board/
  README.md            # quick reference (this protocol, condensed)
  _templates/task.md   # the canonical item template
  tasks/               # one file per item: <ID>-<slug>.md
```

There is **no central index file** to fight over. The "board view" is computed on
demand with `scripts/board/ls.sh` (or plain `grep`/`rg` over frontmatter).

## Item file format

Filename: `<ID>-<kebab-slug>.md`, e.g. `EPIC-003-latency-envelope.md`,
`T-0142-btree-text-index.md`, `BUG-0007-manifest-race.md`.

YAML frontmatter + markdown body:

```yaml
---
id: T-0142
title: B-tree secondary index on text properties
type: task            # epic | story | task | spike | bug
status: ready         # backlog | ready | in_progress | in_review | blocked | done | dropped
priority: P1          # P0 (drop-everything) | P1 | P2 | P3
assignee:             # agent run-id/label, empty if unclaimed
epic: EPIC-005        # parent epic id (epics omit this)
deps: [T-0101]        # ids that must be `done` before this is ready
rubric_refs: [5, 3]   # master-rubric category numbers this advances
estimate: M           # S | M | L  (prefer S — split L)
created: <T+ marker or ISO>
updated: <T+ marker or ISO>
---

## Context
Why this exists; links to the spec/ADR/decision.

## Acceptance criteria
- [ ] concrete, testable bullets — these are what the reviewer checks
- [ ] tests added; coverage not regressed
- [ ] docs/ADR updated if behaviour/architecture changed

## Notes / log
Append-only running notes (who did what, decisions, links to the PR worktree).
```

## IDs

- `EPIC-NNN` — large rubric-aligned bodies of work (seeded; planner may add).
- `STORY-NNN` — a coherent slice of an epic.
- `T-NNNN` — an implementation task.
- `SPIKE-NNNN` — research/design with an open question; output is a decision/spec.
- `BUG-NNNN` — a defect (file these freely; bug-hunting is encouraged).

Allocate the next free number by `ls` + max; collisions are harmless (rename).

## Lifecycle & claiming

`backlog → ready → in_progress → in_review → done` (with `blocked` / `dropped` as
needed). To **claim**: edit the file, set `assignee` + `status: in_progress` +
`updated`. Because each item is its own file, two agents claiming *different* items
never collide. If two agents race the *same* item, last-write-wins and the loser
picks another — keep claims short and check `git status`/re-read before starting.

**Definition of ready / done** lives in
[`autonomous-operating-model.md`](autonomous-operating-model.md). A task isn't
`ready` without acceptance criteria, `rubric_refs`, and satisfied `deps`. A task
isn't `done` until reviewer + pre-mortem sign-off and green checks.

## Dependencies & readiness

- `deps` lists ids that must be `done` first. The planner/pace-marshal flips a
  `backlog` item to `ready` when its deps clear.
- **Design-before-code:** implementation tasks that depend on an unratified design
  carry the relevant `SPIKE`/design task in `deps`; they stay `backlog` until the
  steering committee signs off (esp. `TASK-001` latency envelope, commit protocol).

## Commit discipline

- Board edits are **committed to git** like everything else (small, frequent).
- Prefix board commits `board:` (e.g. `board: claim T-0142`, `board: file BUG-0009`).
- The board is the **source of truth for work**; the rubric reports
  (`.project/reports/`) are the source of truth for *progress*.

## Querying the board (no DB needed)

```bash
scripts/board/ls.sh ready P1        # ready P1 items
scripts/board/ls.sh in_progress     # everything in flight
rg -l 'status: blocked' .project/board/tasks   # find blockers
```

## Hygiene (everyone, continuously)

- Keep `status`/`assignee`/`updated` honest — the grader and pace-marshal trust them.
- Split any `L` you pick up into `S`/`M` children.
- File a `BUG` the moment you find one; don't fix silently.
- When you finish, set `done`, note the landing commit, and pull the next item.
