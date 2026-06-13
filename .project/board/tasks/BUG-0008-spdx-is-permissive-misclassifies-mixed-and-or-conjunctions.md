---
id: BUG-0008
title: SPDX is_permissive misclassifies mixed AND/OR conjunctions as permissive
type: bug
status: in_review
priority: P3
assignee: implementer-wf_e9fceb87-27c-11
epic: EPIC-010
deps: [T-0039]
rubric_refs: [12]
estimate: S
created: T0+0:48
updated: T0+3:18
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
- T0+3:12 (implementer-wf_e9fceb87-27c-11): claimed + implemented TDD-first on
  branch `work/BUG-0008-spdx-precedence-eval` (off latest main). Chose the
  precedence-aware fix: replaced the ` OR `-before-` AND ` substring heuristic
  in `is_permissive` with a recursive-descent SPDX evaluator
  (`OR < AND < WITH < paren/atom`, parens overriding; malformed → conservative
  `false`). 8 new tests (incl. the reported `(MIT OR Apache-2.0) AND GPL-3.0`
  case) RED→GREEN; full suite 131/131 green; `./format_code.sh` clean.
  Set `in_review`; PR.md in the worktree. Awaiting adversarial-reviewer +
  premortem sign-off.
- T0+3:14 (adversarial-reviewer): **approve**. Verified the precedence-aware
  recursive-descent evaluator on branch `work/BUG-0008-spdx-precedence-eval`.
  Every attack (copyleft `AND`-component false-positive, precedence bypass,
  `WITH` smuggling, malformed/illegal-char input) resolves in the conservative
  reject direction — could not construct a permissive misclassification of a
  required copyleft term. `./format_code.sh` exit 0, full `cargo test` green,
  `tests/license_manifest.rs` green against all ~27 real manifest entries (no
  regression). Non-blocking notes: over-strict `Apache-2.0+` handling (safe
  direction) and the pre-existing tracked-PR.md hygiene issue (BUG-0013, out of
  scope). Adversarial-reviewer checkbox ticked in PR.md. Pending premortem.
- T0+3:18 (premortem-analyst): **approve**. Worked backwards through all six
  pre-mortem lenses; the only consequence surface this pure-function change can
  touch is the license guardrail, and every probed failure direction resolves in
  the conservative (reject / fail-loud) direction — no path to a silent
  false-permissive (the one P0 outcome). Re-verified gate checks in the worktree:
  `./format_code.sh` exit 0, `cargo test --lib` 107/107, `tests/license_manifest.rs`
  2/2 against the real (now non-empty) manifest. Diff scope is `src/licenses.rs`
  + `PR.md` only — no `Cargo.toml`/`Cargo.lock` change, so no new dependency to
  vet, no `unsafe`, no concurrency/storage interaction. Filed **BUG-0015** (P3,
  ready) for a latent unbounded-recursion stack-overflow on pathologically deep
  parens (reachable only via the committed manifest; fail-loud, never masks
  copyleft) — accepted as non-blocking follow-up, not a gate. Premortem checkbox
  ticked in PR.md. Both review gates now green; ready for the integrator.
