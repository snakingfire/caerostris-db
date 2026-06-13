---
id: BUG-0029
title: BUG-0023 landed on main before the premortem gate cleared — land.sh two-gate assertion was bypassed or ineffective
type: bug
status: ready
priority: P1
assignee:
epic: EPIC-010
deps: []
rubric_refs: [12]
estimate: S
created: T0+4:10
updated: T0+4:10
---

## Context

Found during the premortem of BUG-0023. The simulated-PR workflow requires **both**
review gates to be `approve` (checkboxes checked in `PR.md`) before the integrator
lands a change:

- `docs/process/simulated-pr-workflow.md` §"Landing": `land.sh` step 1 — "Reads
  `PR.md` — asserts both review-gate checkboxes are signed off."
- `docs/process/adversarial-review-loops.md` §"Process" step 5 — "Both verdicts must
  be `approve` before the integrator is called."
- `scripts/pr/land.sh` lines ~120-133 implement this: it greps for
  `- [x] premortem-analyst sign-off` and exits non-zero ("FAIL: premortem-analyst
  sign-off is not checked in PR.md.") if it is unchecked.

**What actually happened for BUG-0023:** the integrator landed it onto `main` in
commit `a874eadfb4e71d4bfb97a46d9ab27ed7439bfe4d` at T0+4:00 (board note + commit
message `land: BUG-0023 unicode-ident Unicode-3.0 license in deny.toml + manifest`).
But the `- [x] premortem-analyst sign-off` checkbox in `.worktrees/BUG-0023/PR.md`
was still **unchecked** at that time — the premortem-analyst only checked it at
T0+4:10 (after the land). So the change shipped to `main` with only one of the two
mandatory gates cleared.

This means one of:
1. `land.sh` was bypassed (the integrator merged directly, not via the script), or
2. `land.sh` ran but its premortem-checkbox assertion did not fire for this PR
   (e.g. it read a stale/scaffold `PR.md`, a different path, or the grep pattern did
   not match the actual line), or
3. the checkbox was momentarily checked then reverted by a concurrent board/PR.md
   writer (the BUG-0023 board file demonstrably suffered a last-write-wins race
   during this window).

In all three cases the **two-gate invariant is not actually enforced end-to-end**.
The blast radius for BUG-0023 itself was nil (the change was license-gate metadata
and is risk-free — see its premortem verdict), but the *next* time this happens the
unguarded change could touch the ACID commit path, the latency envelope, or
concurrency — exactly the P0 surfaces the premortem gate exists to catch. This is an
`EPIC-010` (harden the autonomous harness) defect: the gate that protects every land
is not provably blocking.

## Acceptance criteria
- [ ] Determine the actual cause: was `land.sh` invoked for BUG-0023, and if so why
      did the premortem-checkbox assertion not block? (Check the integrator's invocation
      path / logs; reproduce by running `scripts/pr/land.sh` against a `PR.md` whose
      premortem checkbox is unchecked and confirm it exits non-zero.)
- [ ] Make the two-gate assertion impossible to bypass for a normal land: e.g.
      `land.sh` re-reads `PR.md` from the worktree tip commit (not the working tree),
      fails closed if either checkbox is absent OR if a `verdict: approve` /
      `**Verdict:** approve` block for that role is missing, and the integrator path
      always routes through `land.sh` (no direct `git merge` to `main`).
- [ ] Add a guard against the checkbox-grep matching a scaffold/unfilled `PR.md`
      (e.g. require the corresponding verdict block to be present and `approve`, not
      just the `[x]`).
- [ ] Regression test: a `tests/`-level or shell-level test that feeds `land.sh` a
      PR.md with (a) both gates checked → allowed, (b) premortem unchecked → refused,
      (c) adversarial unchecked → refused.
- [ ] tests added; coverage not regressed
- [ ] docs updated if the landing protocol changes (`simulated-pr-workflow.md`)
- [ ] `./format_code.sh` green

## Notes / log
- **T0+4:10 — filed by premortem-analyst** during the BUG-0023 premortem. Verified
  `scripts/pr/land.sh` *does* contain the premortem-checkbox assertion (lines
  ~120-133), yet BUG-0023 landed (`a874ead`, T0+4:00) with that checkbox unchecked
  (premortem-analyst signed at T0+4:10, after the land). Pre-existing harness defect;
  not introduced by BUG-0023 — BUG-0023 merely exposed it. Filed against EPIC-010.
