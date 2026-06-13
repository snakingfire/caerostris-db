---
id: BUG-0016
title: PR.md review artifact is committed on main (and not gitignored)
type: bug
status: ready
priority: P3
assignee:
epic: EPIC-010
deps: []
rubric_refs: [12]
estimate: S
created: T+3:20
updated: T+3:20
---

## Context

`PR.md` is a per-worktree review artifact scaffolded by `scripts/pr/open.sh` and
consumed by the review gate. It is meant to live in the work-item worktree only,
never on `main`. Currently `PR.md` is **tracked and committed on `main`** (it
churns from task to task — at time of filing it held a stale T-0022 PR body), and
there is **no `.gitignore` rule** keeping it out. This is a process-hygiene defect
(rubric Cat. 12): review scaffolding leaks into the canonical history and shows
up as a spurious diff in unrelated worktrees (a fresh branch inherits whatever
PR.md happened to be on `main`).

Discovered while landing T-0004 (the worktree inherited a stale T-0014 PR.md).

## Acceptance criteria

- [ ] `PR.md` is removed from `main` (`git rm --cached PR.md` + commit) so it is
      no longer tracked.
- [ ] `.gitignore` gains a `PR.md` rule (and/or `scripts/pr/open.sh` writes it to
      a path already ignored) so it can never be re-committed.
- [ ] `scripts/pr/land.sh` is verified to not depend on a tracked `PR.md`.
- [ ] A repo-hygiene test asserts `PR.md` is untracked on `main` (mirrors the
      existing `tests/repo_hygiene.rs` guards).
- [ ] `./format_code.sh` green.

## Notes / log

- Filed by implementer-wf_fe688db0-093-7 while landing T-0004. Low priority (P3):
  cosmetic/hygiene, no correctness or gate impact. Out of scope for T-0004's three
  deliverables, so split off per the task-board protocol ("file a BUG the moment
  you find one; don't fix silently").
