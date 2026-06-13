---
id: BUG-0015
title: SPDX is_permissive recursive-descent parser has no depth cap (stack overflow on deeply-nested parens)
type: bug
status: ready
priority: P3
assignee:
epic: EPIC-010
deps: [BUG-0008]
rubric_refs: [12]
estimate: S
created: T0+3:18
updated: T0+3:30
---

## Context

Found during the pre-mortem of BUG-0008 (SPDX precedence fix). The new
recursive-descent SPDX evaluator in `src/licenses.rs` (`SpdxParser::parse_atom`
→ `parse_or`) recurses once per `(` with no depth bound. A pathologically
deep expression overflows the stack and aborts the process:

```
is_permissive(&format!("{}MIT{}", "(".repeat(200_000), ")".repeat(200_000)))
// thread 'main' has overflowed its stack
// fatal runtime error: stack overflow, aborting (SIGABRT, rc=134)
```

~5k nesting levels are fine; ~200k aborts.

## Why this is low priority (P3, not a BUG-0008 blocker)

- The **only** caller is `check()` over `parse_manifest(docs/licenses/manifest.toml)`
  — a hand-authored, committed, reviewed file, not network/untrusted input. There
  is no adversary-controlled path to this function.
- No real SPDX expression nests parentheses beyond a handful of levels.
- The failure is **fail-loud** (a crashed license-check test / red CI), never a
  silent false-permissive. It cannot mask a copyleft license — the security-
  relevant direction is unaffected.

Tracked so the evaluator degrades gracefully on absurd input rather than aborting,
which is the more conservative behaviour the module otherwise upholds.

## Acceptance criteria
- [ ] `is_permissive` rejects (returns `false`, conservatively) an expression
      whose parenthesis nesting exceeds a fixed bound (e.g. depth > 64) instead
      of recursing unboundedly — OR converts the recursion to an explicit
      iterative/heap stack.
- [ ] Test: a deeply-nested expression (e.g. 100_000 nested parens) returns
      `false` without aborting the process.
- [ ] Realistic nesting (the existing `nested_parentheses_are_honored` cases,
      and a modest depth like 8–16) still classifies correctly.
- [ ] Existing license tests still green; coverage not regressed.

## Notes / log
- Filed by premortem-analyst during BUG-0008 review (T0+3:18). See the BUG-0008
  PR.md "Pre-mortem Analysis" block ([OPERATIONAL] finding) for the empirical
  repro and the accepted-risk rationale that kept it from blocking BUG-0008.
- T0+3:30 — adversarial-reviewer **approve** on branch `work/BUG-0015-spdx-depth-cap`
  (worktree `.claude/worktrees/wf_e9fceb87-27c-42`, HEAD 8d5ce5a). No blocking
  findings; depth cap (MAX_PAREN_DEPTH=64) verified to bound the sole recursion edge,
  cap fails closed (conservative `false`), boundary pinned by tests, format+tests
  re-run green. Two non-blocking notes recorded in PR.md. Still needs premortem-analyst
  sign-off before landing.
