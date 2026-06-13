---
id: BUG-0018
title: TCK parse-error gap (1602 vs pinned 1615) is unowned and mis-cited as "BUG-0008"
type: bug
status: in_review
priority: P1
assignee: test-author-wf_156e2b80-bb6-44
epic: EPIC-002
deps: []
rubric_refs: [4, 12]
created: T+4:30
updated: T+5:30
---

## Context

Filed by `adversarial-reviewer` while reviewing the T-0005 PR
(`work/T-0005-llvm-cov-coverage-grader-ci`). Two distinct, related defects:

1. **The TCK parse-error gap has no owning board item.** The live `tck-runner`
   reports `total: 1602`, `parse_errors: 1` (one `.feature` file fails to parse),
   whereas the pinned spec everywhere — `master-rubric.md` Cat. 4,
   `docs/process/testing-and-benchmarks.md` §6, `T-0002` acceptance criteria, and
   `.project/decisions/0008-tck-passrate-definition-and-pinning.md` — pins the
   suite to **1615** scenarios (tag `1.0.0-M23`, commit `007895a`) and requires
   `tck::verify_suite_size()` to **abort** if the loaded count differs. T-0002's
   own log (line 49) records that it landed reporting `0/1602` with `1
   parse_error` — i.e. in a state that contradicts its own ACs — but no BUG was
   filed for the parse failure. The gap is real and currently **unowned**.

2. **The gap is mis-cited as "BUG-0008" in durable, grader-facing artifacts.**
   The T-0005 PR commits `.project/reports/tck-latest.json` (`_stub` field) and
   `.project/reports/README.md` (lines 58, 70) that attribute the parse failure
   to "the known BUG-0008 parser gap" and point to `testing-and-benchmarks.md §6`.
   But **BUG-0008 is the SPDX `is_permissive` license-classification bug** — wholly
   unrelated to TCK feature-file parsing — and §6 does not mention the parse gap or
   BUG-0008 at all. The README bills itself as the "machine-readable contract
   between CI and the rubric-grader" and instructs the grader's tamper-check to
   trust that "the gap is the named BUG-0008 corpus file." A grader following that
   instruction is misled. This violates the commander's intent that gaps be "named
   honestly on the board, not hidden."

There is also a deeper pin discrepancy to reconcile (separate concern, captured
here for traceability): the *vendored corpus* on `main`
(`tck/openCypher/PINNED_TAG` = `2024.3`, `PINNED_COMMIT` = `677cbaf`) does not
match the *spec pin* (`1.0.0-M23` / `007895a`). Either the corpus or the spec is
stale; the discrepancy is currently undocumented as a defect.

## Acceptance criteria
- [x] The exact `.feature` file that fails to parse is identified and named in this item:
      **`tck/openCypher/features/expressions/literals/Literals6.feature`** (13 plain
      `Scenario:`, 0 outlines). The `gherkin` 0.16 parser chokes on its heavily-escaped
      result-table cells. Now *machine-checked*, not just commented: new
      `tck_runner::runner::unparseable_features()` + the integration guard
      `tck-runner/tests/vendored_corpus.rs::parse_gap_is_exactly_literals6` assert the gap
      is exactly this one named file (a new unparseable file, or this one closing, fails CI).
- [x] The parse failure is owned: **this BUG (BUG-0018)** now tracks it; the named-file
      guard makes the gap visible and non-silent. Closing the gap (so the 13 scenarios enter
      `total`) is the open remediation tracked here (gherkin upgrade or a vendored fixup).
- [x] All artifacts mis-citing "BUG-0008" for the TCK parse gap are corrected to cite
      `BUG-0018`. Fixed on `main` in this PR: `tck-runner/src/{main,report}.rs`,
      `tck-runner/tests/vendored_corpus.rs`, `.project/decisions/0013-*.md`,
      `.project/reports/tck-T+02-30.md`. **Deferred (not on `main`, owned by the T-0005 PR):**
      `.project/reports/tck-latest.json` `_stub` and `.project/reports/README.md` do not yet
      exist on `main` — they are part of the unlanded `work/T-0005-*` PR; per this item they
      are fix-forward after T-0005 lands, or folded into the T-0005 re-review. The grader
      agent prompt (`.claude/agents/rubric-grader.md` L51-52) is **deferred**: editing an
      agent-definition file is an agent-self-modification action blocked by the harness
      guardrail for a test-author PR; flagged for an authorized follow-up (Decision 0034).
- [x] The vendored-corpus pin (`2024.3` / `677cbaf`) vs spec pin (`1.0.0-M23` / `007895a`)
      discrepancy is reconciled in **Decision 0034**: the vendored `2024.3` / `677cbaf`
      corpus is canonical (it is the corpus actually vendored, run, and graded); the stale
      `caerostris_db::tck` spec pin is updated to match (`PINNED_TCK_TAG = "2024.3"`,
      `PINNED_TCK_COMMIT = "677cbaf…"`, `PINNED_TCK_SCENARIOS = 3884` expanded, with a new
      `PINNED_TCK_SCENARIO_DEFINITIONS = 1615` for traceability). master-rubric Cat. 4,
      testing-and-benchmarks §6, and Decision 0008 (cross-linked as superseded-on-pin) are
      updated; `tests/tck_passrate_contract.rs` + `src/tck.rs` tests assert the new pin.
- [x] docs / ADR updated (Decision 0034 + the spec docs above); `./format_code.sh` green.

## Notes / log
- **T+4:30 — adversarial-reviewer** filed while reviewing T-0005. The numeric
  pass-rate is honest (0/1602) and not gamed; the defect is the *attribution* (wrong
  bug ID, unsupported doc reference) and the *unowned* parse failure. Root cause
  predates T-0005 (baked in when T-0002 landed at T+2:30); T-0005 propagated the
  wrong citation into the grader contract doc, which is why the T-0005 PR is
  returned `changes_requested` to correct the citations alongside this filing.
- **T+5:05 — adversarial-reviewer** (reviewing BUG-0009, `work/BUG-0009-expand-outlines-by-examples`):
  heads-up for the reconciliation here — once BUG-0009 lands, the live harness `total`
  moves **1602 → 3884** and `tck_tag` stays `2024.3` (Scenario Outlines now expanded per
  `Examples` data row, per Decision 0013). The pin/denominator reconciliation in this item
  must therefore target 3884 (the fully-expanded parseable count), not 1602/1615. The
  grader instruction (`.claude/agents/rubric-grader.md` L51-52) still pins `1.0.0-M23` /
  `total == 1615`; reconciling it is part of this BUG's scope.
- **T+4:45 — test-author** (`work/BUG-0018-tck-parse-gap-citations-and-pin-reconcile`):
  implemented TDD-first. (1) Named + machine-guarded the parse file as `Literals6.feature`
  (`unparseable_features()` + `parse_gap_is_exactly_literals6`). (2) Reconciled the pin via
  **Decision 0034** — vendored `2024.3` / `677cbaf` is canonical; updated `caerostris_db::tck`
  to `2024.3` / `677cbaf` / expanded `total = 3884` (+ `PINNED_TCK_SCENARIO_DEFINITIONS = 1615`),
  master-rubric Cat. 4, testing-and-benchmarks §6, Decision 0008 (superseded-on-pin). The
  contract module now agrees with the live harness denominator (3884). (3) Corrected the
  BUG-0008 → BUG-0018 mis-citations in code/tests/decisions/reports on `main`. Two artifacts
  are deferred and noted in AC#3 (the T-0005 PR's `tck-latest.json`/`README.md`, not yet on
  `main`; and the grader agent prompt, blocked as agent-self-modification — flagged in
  Decision 0034 for an authorized follow-up). Note: a concurrent lane holds a near-identical
  branch `work/BUG-0018-tck-parse-gap-citation-and-pin-reconciliation`; the integrator
  serializes — first clean land wins, the other rebases.
- **T+5:15 — adversarial-reviewer** verdict **changes_requested** on branch
  `work/BUG-0018-tck-parse-gap-citation-and-pin-reconciliation` (PR.md in worktree
  `wf_fe688db0-093-30`). The technical reconciliation is correct and tests pass (367/367).
  Four blocking process/GATE integrity findings: (1) pin change justified by false
  "2024.3 is 1.0.0-M23 renamed" claim that contradicts upstream tag history and
  Decision 0008 lines 59-63; must reframe as deliberate pin bump per Decision 0008
  L61-63; (2) steering re-ratification required before overriding the Cat. 4 GATE
  canonical pin (Decision 0008 is a steering-ratified decision, Loop A first); (3) the
  rewritten 100% bar allows 13 Literals6 `parse_errors` scenarios as a permanent
  excluded subset — add `parse_errors == 0` to the Cat. 4 GATE requirement; (4) the
  headline P0 (grader misfire on `rubric-grader.md` L51-52 still citing 1.0.0-M23/1615)
  is undelivered — this PR does not touch that file, so the false-P0 persists.
  Review-gate checkbox left unchecked. Branch returned to author.
- **T+5:15 — integrator**: LANDING BLOCKED — adversarial-reviewer sign-off is
  `changes_requested`. Review-gate checkbox unchecked. Required actions before reland:
  (1) get steering sign-off (Loop A) on the pin bump; (2) correct the "calendar rename"
  claim to "deliberate pin bump"; (3) add `parse_errors == 0` to Cat. 4 GATE 100% bar;
  (4) update `.claude/agents/rubric-grader.md` L51-52 with the new pin (or obtain
  explicit authorization for that agent-self-modification edit). After fixes, re-run
  `./format_code.sh` + cargo nextest, reset review-gate checkboxes to unchecked, and
  request a fresh adversarial review + pre-mortem pass. Status set to `blocked`.
- **T+5:18 — adversarial-reviewer** (`work/BUG-0018-tck-parse-gap-citations-and-pin-reconcile`):
  **APPROVE** on this branch. Non-blocking observations recorded in PR.md. BUG-0027 filed.
- **T+5:30 — integrator** (reland attempt): premortem-analyst checkbox unchecked. Status
  remained `blocked`. Required premortem sign-off before landing.
- **T+5:35 — premortem-analyst** (reland dispatch): **APPROVE**. All failure modes guarded:
  pin drift (compile-time const-block), gap silent-close (named guard), new unparseable file
  (count guard), grader mis-fire (pre-existing, BUG-0027 owned). No new unguarded risks.
