# Decision 0008 — TCK pass-rate definition (pending in denominator) and release pinning

- **Date / marker:** T0 (2026-06-13T18:24:00Z)
- **Owner:** steering-query-cypher
- **Status:** recorded; tracked by BUG-0007 (P0)
- **Rubric:** Cat. 4 (openCypher/TCK), Cat. 10

## Context

Cat. 4 scores "TCK pass-rate %" with a 100% GATE bar. T-0002 emits
`{ total, pass, pending, fail, pass_rate: P/N }` with unimplemented features as
`pending`.

## Finding

Two ambiguities make "100%" gameable:

1. **Denominator.** "pass-rate" + a separate `pending` bucket invites
   `pass_rate = pass / (pass + fail)`, excluding `pending`. That hides
   incompleteness and is a curated subset by another name — a falsification of
   "100% means all of it, not a subset" (commanders-intent.md L31).
2. **Suite identity / drift.** "100% of the TCK" is undefined without a pinned
   release tag and a recorded `total`; otherwise the score can rise by dropping
   `.feature` files.

## Decision

- Mandate **`pass_rate = pass / total`, `total = pass + pending + fail`**, no
  scenario excluded from `total`; 100 requires `pending == 0 && fail == 0`.
  Moving a scenario to `pending` to inflate the rate is forbidden.
- Pin a specific openCypher TCK release tag; record the tag and its expected
  `total` scenario count. Harness emits both; a guard test fails if the loaded
  count differs from the recorded pinned `total`.
- The rubric-grader cron must read `pass/total`.

## Alternatives considered

- **`pass/(pass+fail)` with `pending` excluded.** Rejected (see finding 1).
- **Track latest TCK `main` instead of a pinned tag.** Rejected: non-reproducible
  grading; a moving target cannot anchor a GATE. Bumping the pin later is a
  deliberate, recorded action.

## Consequences

Rubric Cat. 4 wording and T-0002 acceptance criteria amended; grader reads the
documented field. Reproducible, non-gameable Cat. 4 metric.
