---
id: BUG-0013
title: Stray PR.md committed to main (should be gitignored, not tracked)
type: bug
status: in_review
priority: P3
assignee: implementer-wf_156e2b80-bb6-13
epic: EPIC-010
deps: []
rubric_refs: [12]
estimate: S
created: 2026-06-13T20:45:00Z
updated: 2026-06-13T21:39:00Z
---

## Context

Spotted by `implementer-wf_86b0c2e8-f29-13` while opening BUG-0011's PR. A root-level
`PR.md` is **tracked on `main`** (added by T-0039 commit `f67868b` — "board: T-0039
in_review; add PR.md"). `PR.md` is a per-worktree artifact that `scripts/pr/open.sh`
writes under `.worktrees/<ID>/PR.md`, and `.gitignore` already ignores `.worktrees/`.
A root `PR.md` should never be committed: it is transient per-PR scratch, and a tracked
copy on `main` means every new worktree inherits a stale PR description from whatever
task last committed it (BUG-0011's worktree inherited T-0039's PR.md verbatim).

Severity is low: it is process-hygiene (Cat. 12), not a GATE. No code or behaviour is
affected. Filed rather than fixed-in-place because removing a tracked file from `main`
is an integrator-landed change of its own and is out of scope for BUG-0011 (which is a
docs cross-reference sweep). BUG-0011's PR overwrites the stray file's *contents* with
its own description on its branch, but the underlying "PR.md is tracked at all" defect
on `main` remains until this bug lands.

## Acceptance criteria
- [x] Root-level `PR.md` is removed from git tracking on `main` (`git rm --cached PR.md`
      via a landed PR; do not delete worktree-local copies that workers actively edit).
- [x] `.gitignore` ignores a root-level `PR.md` (and/or `open.sh` is confirmed to only
      ever write `.worktrees/<ID>/PR.md`, which is already ignored) so it cannot be
      re-committed by accident.
- [x] A guard (extend `tests/repo_hygiene.rs`) asserts `PR.md` is not tracked at the repo
      root, so the regression is caught in CI.
- [x] `./format_code.sh` green.

## Notes / log
- 2026-06-13T20:45:00Z (implementer-wf_86b0c2e8-f29-13): filed during BUG-0011. The
  tracked file is `PR.md` at the repo root, introduced by `f67868b`. Same docs/process
  hygiene family as BUG-0003/BUG-0010/BUG-0011. Low priority; does not block any GATE.
- 2026-06-13T21:35:00Z (implementer-wf_156e2b80-bb6-13): claimed; implemented TDD-first
  on `work/BUG-0013-stray-pr-md-committed-to-main-should-be-gitignored`, rebased onto
  latest `main` (`d3b357f`). Fix commit `9bfe14d`: `git rm --cached PR.md` (untrack),
  `.gitignore` `/PR.md` rule, and two `tests/repo_hygiene.rs` guards
  (`root_pr_md_is_not_tracked`, `gitignore_ignores_root_pr_md`) — both RED before the
  fix, GREEN after. Confirmed `scripts/pr/open.sh` only ever writes
  `.worktrees/<ID>/PR.md` (no script change). `cargo nextest run` 125/125 green;
  `./format_code.sh` exit 0. PR.md filled; status → `in_review`; dispatching
  adversarial-reviewer + premortem-analyst.
- 2026-06-13T21:39:00Z (adversarial-reviewer): **APPROVE**. Verified branch-tip tree
  omits `PR.md` (`git ls-tree`/`cat-file`), `/PR.md` gitignore correctly anchored
  (no over-match on nested/substring), guard scoped to root `PR.md` and live in CI
  (real git checkout as runner user — verified RED→GREEN), `format_code.sh` exit 0
  with no fmt drift, clippy `-D warnings` clean, full `cargo test` green, rebase clean
  vs current `main` (disjoint files; `main` still tracks `PR.md`). No secrets/deps/
  history-rewrite. Two non-blocking notes (provenance imprecision re `f67868b`; test
  skip-path hardening) recorded in PR.md. adversarial-reviewer box ticked. Awaits
  premortem-analyst.
