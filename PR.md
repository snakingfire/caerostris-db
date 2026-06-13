# PR: BUG-0008 — SPDX is_permissive misclassifies mixed AND/OR conjunctions as permissive

## Board item

[.project/board/tasks/BUG-0008-spdx-is-permissive-misclassifies-mixed-and-or-conjunctions.md](.project/board/tasks/BUG-0008-spdx-is-permissive-misclassifies-mixed-and-or-conjunctions.md)

Branch: `work/BUG-0008-spdx-precedence-eval` (based on the latest `main`).

## Rubric refs

Cat 12 (Engineering & process health — license hygiene / open-source guardrails).

## Acceptance criteria (from board item)

- [x] `is_permissive` honors SPDX operator precedence (`AND` binds tighter than
      `OR`) and parenthesized grouping — OR `is_permissive` conservatively
      *rejects* (returns `false` / surfaces a violation) any expression that
      mixes ` AND ` and ` OR `, rather than guessing. → Chosen the *precedence-aware*
      branch: recursive-descent evaluator with `OR < AND < WITH < atom`,
      parentheses overriding. Tests `and_binds_tighter_than_or_without_parens`,
      `nested_parentheses_are_honored`.
- [x] Test: `(MIT OR Apache-2.0) AND GPL-3.0` is classified non-permissive. →
      `parenthesized_or_anded_with_copyleft_is_not_permissive`.
- [x] Test: `(MIT OR GPL-3.0) AND Apache-2.0` is classified permissive only if
      every conjunct is satisfiable by an approved token. →
      `parenthesized_disjuncts_anded_are_permissive_when_each_satisfiable`
      (permissive) + `..._not_permissive_when_one_unsatisfiable` (the negative).
- [x] Existing license tests still green; coverage not regressed. → All 13
      pre-existing license tests pass; 8 new tests added exercising every branch
      of the new evaluator.

## Summary of change

`is_permissive` in `src/licenses.rs` previously string-scanned for ` OR ` *before*
` AND `, so any expression containing an ` OR ` was treated as a pure disjunction
and its ` AND ` operands were silently ignored. This masked copyleft `AND` components
(e.g. `(MIT OR Apache-2.0) AND GPL-3.0`) as permissive — the dangerous direction,
since an `AND` component is a license you are *required* to comply with.

This change replaces the substring heuristic with a small recursive-descent
evaluator for the SPDX license-expression grammar (the subset relevant to the
allow-list check): `OR` (lowest precedence) < `AND` < `WITH` < parenthesized group
/ token, matching the official SPDX precedence (`AND` binds tighter than `OR`;
parentheses override). The evaluator computes a single boolean — "is this
expression satisfiable using only approved tokens": a token is permissive iff it is
on the approved allow-list; an `AND` node is permissive iff *all* operands are; an
`OR` node iff *any* operand is. `WITH` (license-exception) tokens are conservatively
rejected since no exception-bearing identifier is on the allow-list. Malformed
expressions (unbalanced parens, empty or dangling operands) are conservatively
rejected (`false`) rather than guessed. The legacy `A/B` slash form is still
normalized to `A OR B` first. Behaviour is otherwise unchanged for the
previously-handled cases, so all prior tests stay green.

## Test evidence

**TDD:** the 8 new tests were written first and run RED against the buggy
`is_permissive` (4 failed, incl. `parenthesized_or_anded_with_copyleft_is_not_permissive`
on the exact reported case `(MIT OR Apache-2.0) AND GPL-3.0`), then GREEN after
the evaluator was implemented.

`cargo nextest run` (full suite, with the local S3 mock env up via
`scripts/env/up.sh` + `scripts/env/bucket.sh BUG-0008`):

```
Summary [4.981s] 131 tests run: 131 passed, 0 skipped
```

`cargo nextest run licenses` (focused — 13 pre-existing + 8 new):

```
Summary [0.545s] 21 tests run: 21 passed, 110 skipped
```

(One spurious nextest `LEAK` flag appeared on
`malformed_expressions_are_conservatively_rejected` in the parallel run; the test
passes, is a pure function with no I/O/threads/FDs, and runs clean in isolation —
inherited-handle noise, not a real failure.)

`./format_code.sh` (cargo fmt + clippy `-D warnings` + taplo): **green, exit 0**.

Coverage: `cargo llvm-cov` is unavailable outside the Nix shell here
(`llvm-tools-preview` not installed), so the % is measured in CI. Not regressed by
inspection: the change is one self-contained module; every branch of the new
recursive-descent evaluator (OR/AND/WITH/paren/ident, malformed, whitespace/case)
is exercised by a dedicated test.

Diff: `src/licenses.rs` only, +249/-20 (single file, under the 300-line guidance).

## Review gate

- [ ] adversarial-reviewer sign-off (see docs/process/adversarial-review-loops.md)
- [ ] premortem-analyst sign-off (see docs/process/adversarial-review-loops.md)
- [x] `./format_code.sh` green
- [x] `cargo nextest run` green (or `cargo test` outside Nix shell)
- [x] coverage not regressed
- [x] board item updated to `in_review`

<!-- Reviewers: append your verdict block below this line per adversarial-review-loops.md -->
