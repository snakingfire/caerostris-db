---
id: BUG-0008
title: SPDX is_permissive misclassifies mixed AND/OR conjunctions as permissive
type: bug
status: in_progress
priority: P3
assignee: implementer-wf_e9fceb87-27c-11
epic: EPIC-010
deps: [T-0039]
rubric_refs: [12]
estimate: S
created: T0+0:48
updated: T0+3:07
---

## Context

Found during adversarial review of T-0039 (license-manifest check). The
`is_permissive` function in `src/licenses.rs` checks for the substring ` OR `
*before* ` AND `, so any expression containing an ` OR ` is treated as a pure
disjunction — the ` AND ` operands are ignored.

Reproduced:

```
is_permissive("(MIT OR Apache-2.0) AND GPL-3.0") == true   // WRONG: GPL-3.0 is a required component
```

The dangerous direction is masking a copyleft `AND` component (a license you are
*required* to comply with) as permissive. SPDX `AND` binds tighter than `OR`,
and parentheses change grouping — neither is honored.

## Why this is not a T-0039 blocker

- The manifest is empty today (zero third-party dependencies), so the code path
  is unreachable in the current repo.
- `cargo-deny` (layer 2, `deny.toml`) reads each crate's real license metadata
  and would reject a GPL-encumbered crate independently of this hand-rolled
  parser.

Tracking it so it is fixed **before the first real dependency with a compound
SPDX expression is recorded** in the manifest.

## Acceptance criteria
- [ ] `is_permissive` honors SPDX operator precedence (`AND` binds tighter than
      `OR`) and parenthesized grouping — OR `is_permissive` conservatively
      *rejects* (returns `false` / surfaces a violation) any expression that
      mixes ` AND ` and ` OR `, rather than guessing.
- [ ] Test: `(MIT OR Apache-2.0) AND GPL-3.0` is classified non-permissive.
- [ ] Test: `(MIT OR GPL-3.0) AND Apache-2.0` is classified permissive only if
      every conjunct is satisfiable by an approved token.
- [ ] Existing license tests still green; coverage not regressed.

## Notes / log
- Filed by adversarial-reviewer during T-0039 review. See the T-0039 PR.md
  "Adversarial Review" block for the reproduction and rationale.
