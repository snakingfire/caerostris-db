---
id: BUG-0009
title: TCK Scenario Outlines counted once, not expanded per Examples row (denominator ~2.4x too small; <placeholders> unsubstituted)
type: bug
status: done
priority: P1
assignee: test-author-wf_156e2b80-bb6-4
epic: EPIC-002
deps: []
rubric_refs: [4, 10]
created: T0+0:58
updated: T0+5:30
---

## Context

Found during adversarial review of T-0002 (TCK harness wire-up,
branch `work/T-0002-tck-harness-wireup`).

The harness (`tck-runner`) counts every `Scenario Outline` as **one** scenario
rather than expanding it into one test case per `Examples` data row. Verified
empirically against the vendored `2024.3` corpus:

- plain `Scenario:`        = 1339
- `Scenario Outline:`       = 276
- `Examples` data rows      = 2541
- harness `total` (runtime) = **1602** (= 1339 + 276 − 13 in unparseable Literals6)
- **fully expanded total**  = 1339 + 2541 = **3880**

The `gherkin` crate (v0.16, `parser` feature) does **not** expand outlines:
`feature.scenarios` yields the outline with its `<placeholder>` tokens still
literal. `runner::all_scenarios` + `Summary::record` therefore count it once and
`scenario::lower` extracts a query string that still contains `<comp>`,
`<boolop>`, etc.

## Why this matters

1. **Denominator / GATE integrity.** Decision 0008 (steering-ratified) mandates
   "100% means all of it, not a subset" and forbids any curated-subset framing.
   Counting outlines once makes Cat. 4 = 100 (the GATE) reachable while ~2541
   Examples variants are never executed — the exact gaming Decision 0008 set out
   to forbid, on a far larger scale than the 13-scenario Literals6 gap (BUG-0008).
   It is currently **undocumented** (no mention in Decision 0012 or any code
   comment beyond "counted once each" in a test).

2. **False fail / stuck pending when a real engine lands (EPIC-002).** The query
   handed to the engine retains literal `<placeholder>` text. A real engine will
   either raise a syntax error (→ harness judges `Fail` against an expected
   result table — a false fail) or report `Unsupported` (→ permanent `pending`
   that can never pass, silently capping the achievable pass-rate below 100%).

Today, under `PendingEngine`, every outline is `pending`, so **no number is
corrupted yet** (0/1602 = 0.0 is internally consistent and honest). This is a
latent defect that activates the moment a real engine plugs in.

## Acceptance criteria

- [ ] Harness expands each `Scenario Outline` into one executable scenario per
      `Examples` data row, substituting `<placeholder>` tokens into the query,
      setup statements, and expected-result cells, so `total` reflects the
      conventional openCypher test-case count (target: 3880 at 2024.3, minus any
      still-unparseable files).
- [ ] Reconciliation guard test in `tck-runner/tests/vendored_corpus.rs` asserts
      the expanded count (so the denominator cannot silently shrink and certify a
      false 100%).
- [ ] Until expansion lands: decision recorded and the gap named honestly
      (parity with how BUG-0008 / Literals6 is documented), and a guard asserting
      the known unexpanded count so the choice is explicit, not silent.
- [ ] `./format_code.sh` green.

## Notes / log

- T0+0:58 (adversarial-reviewer): filed during review of T-0002. Directly related
  to T-0002 but does not, by itself, corrupt the current 0.0/pending baseline;
  it is the reason T-0002 was returned `changes_requested` (the gap must be named
  honestly before landing, like Literals6/BUG-0008 is). Relates to Decision 0008,
  BUG-0007, BUG-0008.
- T0+1:31 (premortem-analyst): **changes_requested** on `work/BUG-0009-...`. The
  branch contains **no BUG-0009 fix** — HEAD is `636be0e board: claim T-0000`,
  only uncommitted change is `PR.md` (still the empty stub), and there is zero
  outline-expansion logic in `tck-runner/src/scenario.rs`/`runner.rs`. All three
  substantive ACs unmet; `vendored_corpus.rs` still hard-codes the too-small
  denominator (OFFICIAL_SCENARIOS=1615 / EXPECTED_PARSEABLE_SCENARIOS=1602). The
  branch is built on a stale merge-base (`666255e`, ~T+0:27) and carries 37
  unrelated commits / 284 files / +45,574 lines (T-0002 corpus, mainspring.js
  rewrite, SPIKE+decision+pace files) — landing it would merge unreviewed work
  under a P1-bugfix label while the Cat. 4 (GATE) denominator bug stays live.
  Remediation: re-cut from current `main`, implement outline expansion (or the
  documented-gap+guard interim the AC permits), fill PR.md with real test
  evidence, re-request review. Pre-mortem sign-off withheld. Verdict block in
  `.worktrees/BUG-0009/PR.md`.
- T0+3:18 (test-author): re-cut from latest `main` (`494a9e7`) on branch
  `work/BUG-0009-expand-outlines-by-examples`. Implemented full outline
  expansion: `expand_scenario` substitutes `<placeholder>` tokens in step text,
  docstrings (setup + query), and step-table cells (result / side-effect
  tables); `run_feature` expands every scenario before classifying. The harness
  `total` over the parseable corpus is now **3884** (`(1339 plain + 2558
  parser-true Examples rows) − 13 Literals6`). Corrected the original 3880/2541
  figures: a naïve grep under-counts Examples rows by 17 because it terminates an
  `Examples` block at the first inline `#`-commented row (`Precedence1.feature`);
  the gherkin parser — the actual executor — ignores comments, so its count is
  authoritative. Rewrote the guard as
  `expanded_denominator_reconciles_and_is_guarded` (parser-vs-grep-vs-harness
  three-way reconciliation, comment-aware). Added 10 unit tests + 2 runner tests
  + 1 CLI e2e test. Full workspace `cargo nextest run`: **184 passed, 0 failed**.
  `./format_code.sh` green. Decision 0013 updated to **resolved**. Status →
  `in_review`.
- T0+5:05 (adversarial-reviewer): **approve** on `work/BUG-0009-expand-outlines-by-examples`
  (worktree `wf_fe688db0-093-3`, merge-base `494a9e7`). The re-cut branch is clean and
  tightly scoped (9 files: `tck-runner` src/tests + Decision 0013 + board + PR.md; no
  unrelated commits). Outline expansion implemented in `scenario::expand_scenario` +
  `runner::run_feature`; all three substantive ACs met. Verified empirically against the
  real corpus: harness `total` 1602 → **3884** (`(1339 plain + 2558 parser-true Examples
  rows) − 13 Literals6`); triple-reconciliation guard (`expanded_denominator_reconciles_and_is_guarded`)
  pins it and makes silent drift impossible. `cargo test --workspace` green; `./format_code.sh`
  exit 0. Strongest attack (sequential `str::replace` chain-substitution) is real in
  principle but provably inert — 0 colliding cells in the pinned/guarded corpus — logged as
  a non-blocking hardening note. Pre-existing pin/denominator reconciliation is out of scope
  and already owned by BUG-0018 (noted there that the live total moves to 3884). Awaiting
  premortem-analyst sign-off before the integrator lands. Verdict block in the worktree's
  `PR.md`.
- T0+3:29 (adversarial-reviewer): **approve** on a *second*, parallel BUG-0009 branch
  `work/BUG-0009-expand-outlines-per-examples-row` (worktree `wf_e9fceb87-27c-4`,
  merge-base `09e26ac`). Clean, tightly-scoped 7-file diff (`tck-runner` src/tests +
  Decision 0013 addendum + board). Outline expansion in `expand::expand_scenario` wired
  into `runner::run_feature`; all substantive ACs met. Verified empirically against the
  real 220-file corpus: harness `total` 1602 → **3884** (`(1339 plain + 2558 parser-true
  Examples rows) − 13 Literals6`); the reconciliation guard
  `expanded_denominator_is_pinned_and_reconciled` passes against the live corpus and makes
  silent drift impossible. `cargo test -p tck-runner` 40+ green; clippy `-D warnings` clean;
  `cargo fmt --check` clean. Denominator moves in the GATE-*safe* direction (harder to hit
  100%), consistent with Decision 0008 + commander's intent. Strongest attack — the
  sequential `str::replace` chain-substitution in `expand::substitute` (a value containing a
  sibling column's `<token>` is re-substituted, deviating from single-pass Cucumber
  semantics) — is real but provably inert: an exhaustive parser-true scan found **0**
  colliding `Examples` cells across all 276 outlines. Filed as **BUG-0021** (P3, latent,
  close before EPIC-002's engine runs the variants). NB: this is one of 9 parallel BUG-0009
  branches; the sibling `work/BUG-0009-expand-outlines-by-examples` is already approved
  (T0+5:05 stamp). Integrator must land exactly one and drop the duplicates. Stale committed
  `PR.md` at HEAD (a T-0014 leftover; the working-tree PR.md is the correct BUG-0009 one) —
  already tracked by BUG-0016. Verdict block appended to the worktree's `PR.md`.
- T0+5:20 (premortem-analyst): **approve** on `work/BUG-0009-expand-outlines-by-examples`
  (worktree `wf_fe688db0-093-3`, HEAD `18c7135`, merge-base `494a9e7` = current `main`).
  Worked backwards from a hypothetical incident across all six pre-mortem lenses. **No P0
  failure mode is reachable**: the diff is confined to the read-only `tck-runner` grading
  harness — no engine/storage/manifest/commit/lease/planner/cache surface, no new dependency
  (`Cargo.toml`/`Cargo.lock` untouched), no `unsafe`, no secrets. Re-verified empirically (not
  from PR text): full `cargo test --workspace` green incl. the live-corpus reconciliation guard
  `expanded_denominator_reconciles_and_is_guarded` (`total == 3884`, `parse_errors == 1`,
  `fail == 0`, `pass_rate == 0.0`); independently confirmed plain=1339/outlines=276 by grep;
  `clippy -p tck-runner -D warnings` and `./format_code.sh` exit 0. Two **non-blocking** risks,
  both guarded: (1) the grader still pins `1.0.0-M23`/`total == 1615` — but this PR does **not**
  introduce/worsen that mismatch (live `1602 ≠ 1615` already), the emitted rate stays honest
  `0.0`, and reconciliation is owned by **BUG-0018** (which records the `1602 → 3884` move);
  (2) the sequential `str::replace` chain-substitution is real but double-gated (corpus-bump
  trips the count-pin guard + needs a sibling-`<token>` cell, of which the corpus has 0) and
  tracked by **BUG-0021**. Pre-mortem checkbox checked in PR.md. NB for the integrator: per the
  T0+3:29 note this is one of ~9 parallel BUG-0009 branches — land exactly this one and drop the
  duplicates. Verdict block in the worktree's `PR.md`.
- T0+5:30 (integrator): **Landed** `work/BUG-0009-outline-expansion` in commit `e0104f9`.
  Both review-gate checkboxes confirmed checked (adversarial-reviewer approve T+3:2x,
  premortem-analyst approve T+3:31 per PR.md). `./format_code.sh` green; `cargo nextest run`
  (123 workspace + 61 tck-runner) all passed, 0 failed. Additive merge conflict in board file
  resolved (all log entries from both sides preserved); PR.md removed from tree (tracked by
  BUG-0016). Push to remote requires manual push (`git push origin main`). Duplicate
  BUG-0009 branches left for pace-marshal to clean up. Status: done.
