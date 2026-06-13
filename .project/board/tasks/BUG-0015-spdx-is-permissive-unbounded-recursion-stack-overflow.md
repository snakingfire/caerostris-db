---
id: BUG-0015
title: SPDX is_permissive recursive-descent parser has no depth cap (stack overflow on deeply-nested parens)
type: bug
status: done
priority: P3
assignee: implementer-wf_e9fceb87-27c-42
epic: EPIC-010
deps: [BUG-0008]
rubric_refs: [12]
estimate: S
created: T0+3:18
updated: T0+4:40
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
- T0+3:52 — Claimed by implementer-wf_e9fceb87-27c-42. Worktree on branch
  `work/BUG-0015-spdx-depth-cap` (canonical `work/BUG-0015-...` prefix; the
  long auto-slug branch from a stale, zero-commit scaffold was avoided). TDD-first
  fix: cap parenthesis nesting depth in `SpdxParser` at 64 and return a
  conservative `false` past the cap.
- T0+4:01 — PR opened (`in_review`). Fix landed in commit 80deedc on
  `work/BUG-0015-spdx-depth-cap`: `MAX_PAREN_DEPTH = 64`, depth-bounded
  `parse_atom`. Full suite 278/278 green; `./format_code.sh` green; 4 new
  BUG-0015 tests + existing nesting tests pass. PR.md filled. Awaiting
  adversarial-reviewer + premortem-analyst sign-off.
- T0+4:08 — premortem-analyst **approve** on the same branch
  (`work/BUG-0015-spdx-depth-cap`, worktree `wf_e9fceb87-27c-42`, HEAD 8d5ce5a).
  Worked the corruption/SLA/concurrency/error/operational/security lenses: change
  is confined to pure build-time license-parsing logic, so the P0 incident classes
  (ACID, latency-theorem, split-brain, data loss) are structurally unreachable; the
  one behaviour change is fail-loud `false`, never fail-open onto a copyleft license.
  Re-ran `cargo test --lib licenses` (28/28) and `./format_code.sh` (exit 0); zero
  dependency changes. Both review-gate boxes now checked in PR.md — ready to land.
  Two non-blocking notes: (1) a future *generated* manifest over-nesting >64 levels
  would fail loud (acceptable); (2) **duplicate work** — a second lane built an
  equivalent fix on `work/BUG-0015-spdx-is-permissive-recursive-descent-parser-has-no`
  (worktree `wf_156e2b80-bb6-50`). Integrator should land **one** (recommend this
  `spdx-depth-cap` branch) and drop the other.
- T0+4:40 — Landed in commit a8f7ede at T0+4:40. Integrator rebased branch onto
  main (resolving additive board-file conflicts), ran ./format_code.sh (exit 0)
  and cargo nextest run (346/346 passed), then merged via git merge --no-ff.
  Landing commit: a8f7ede. Status: done.
