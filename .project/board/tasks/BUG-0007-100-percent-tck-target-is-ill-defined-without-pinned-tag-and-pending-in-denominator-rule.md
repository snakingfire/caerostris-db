---
id: BUG-0007
title: 100 percent TCK target is ill-defined without pinned tag and pending-in-denominator rule
type: bug
status: blocked
priority: P0
assignee:
epic: EPIC-002
deps: []
rubric_refs: [4, 10]
estimate: S
created: 2026-06-13T18:24:00Z
updated: 2026-06-13T19:45:00Z
---

## Context

Filed by `steering-query-cypher` during the launch ratification pass over
`docs/commanders-intent.md` and `docs/requirements/master-rubric.md`.

Cat. 4 says "Score = TCK pass-rate %" and "100% ... no skipped scenarios"
(master-rubric.md L61-64). `T-0002` defines the harness output as
`{ total, pass, pending, fail, pass_rate: P/N }` and counts `pass / pending /
fail` separately, with unimplemented features bucketed as `pending`.

Two ambiguities make "100%" gameable and therefore not a well-defined GATE:

1. **`pending` and the denominator.** If `pass_rate = pass / total` with
   `total = pass + pending + fail`, then `pending` correctly *depresses* the score
   (good — incompleteness is visible). But the wording "pass-rate" plus a separate
   `pending` bucket invites the back-door reading `pass_rate = pass / (pass+fail)`,
   which **excludes `pending` from the denominator** — that is a curated subset by
   another name and a direct falsification of "100% means all of it, not a subset"
   (commanders-intent.md L31, master-rubric.md L26). The rubric/harness must
   mandate **`pass_rate = pass / total`, with both `pending` and `fail` in the
   denominator**, and forbid moving scenarios to `pending` to inflate the rate.

2. **TCK version pinning + drift.** The official TCK grows release to release.
   "100% of the TCK" is meaningless without a pinned release tag, and the
   `total` count is the rubric's own integrity check (T-0002 already requires
   "scenario count matches the official TCK count for the pinned release"). The
   pinned tag and its scenario `total` must be recorded so the grader can detect
   silent shrinkage of the suite (dropping `.feature` files to raise the rate).

**Why blocking:** Cat. 4 is a GATE that must reach exactly 100. A metric that is
ambiguous about its denominator or its suite cannot be a credible GATE — the grade
could read 100 while a class of scenarios is quietly excluded. **Does NOT block
launch**; it constrains how T-0002 computes and reports the number.

## Acceptance criteria

- [ ] Rubric Cat. 4 and T-0002 amended to state explicitly:
      `pass_rate = pass / total`, `total = pass + pending + fail`, no scenario
      excluded from `total`; reaching 100 requires `pending == 0 && fail == 0`.
- [ ] A specific openCypher TCK release tag is pinned and recorded (in T-0002 and a
      decision doc); the expected `total` scenario count for that tag is recorded.
- [ ] Harness emits the pinned tag and `total` in its machine-readable output so
      the rubric grader can assert the suite was not shrunk.
- [ ] A guard test fails if the loaded scenario count differs from the recorded
      pinned `total` (catches accidental or deliberate suite shrinkage).
- [ ] `./format_code.sh` green.

## Notes / log
- T0 `steering-query-cypher`: filed during ratification. Decision recorded at
  `.project/decisions/0008-tck-passrate-definition-and-pinning.md`. Coordinate
  with the `rubric-grader` cron so it reads `pass/total`, not `pass/(pass+fail)`.
- T+~1:45 `integrator`: LANDING BLOCKED — rebase conflict on `src/lib.rs`.
  The branch was cut before BUG-0006 landed (which added `pub mod query;` to
  `src/lib.rs`). The branch adds `pub mod tck;` at the same location. The two
  hunks are not automatically resolvable by rebase (different module additions
  at the same anchor line). Both reviewers signed off at base `3a9d645` (main tip
  at review time); main has since advanced 2 commits (BUG-0006 landing). Resolution
  required: worker must `git rebase main` in the worktree, resolve the `src/lib.rs`
  conflict (keep BOTH `pub mod query;` AND `pub mod tck;`), re-run
  `./format_code.sh` + `cargo nextest run`, and re-request review (reset both
  review-gate checkboxes to unchecked per the protocol).
  Conflict file: `src/lib.rs` — main has `pub mod query;`, branch adds `pub mod tck;`.
  Fix: both lines must be present in the rebased tree.
