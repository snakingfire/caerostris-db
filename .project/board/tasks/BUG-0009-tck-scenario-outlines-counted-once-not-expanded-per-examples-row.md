---
id: BUG-0009
title: TCK Scenario Outlines counted once, not expanded per Examples row (denominator ~2.4x too small; <placeholders> unsubstituted)
type: bug
status: in_review
priority: P1
assignee: test-author-wf_156e2b80-bb6-4
epic: EPIC-002
deps: []
rubric_refs: [4, 10]
created: T0+0:58
updated: T0+3:05
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
- T0+3:05 (test-author): re-cut clean from latest `main` (`494a9e7`) on branch
  `work/BUG-0009-outline-expansion`. Implemented the **real fix** (not the
  interim): `tck_runner::outline::expand_scenario` expands each `Scenario
  Outline` into one concrete scenario per `Examples` data row, substituting
  `<placeholder>` tokens into the name, every step value, every docstring
  (query + setup), and every data-table cell; `runner::all_scenarios` expands
  before `classify`. Harness `total` is now the expanded **3884** (= 1326
  parseable plain + 2558 expanded outline cases), up from the unexpanded 1602.
  Rewrote the `vendored_corpus.rs` guard
  (`outline_expansion_total_is_reconciled`) to pin the expanded denominator from
  the authoritative `gherkin` parser and assert no `<placeholder>` survives.
  Found+fixed a latent counter bug: the old grep heuristic dropped 17
  commented-out (`#| |`) example rows in `Precedence1.feature` (so the corrected
  example-row count is 2558, not 2541). Decision 0013 updated to **RESOLVED**.
  61 tck-runner tests (+11), 184 workspace, all green; `./format_code.sh` green.
  Status → `in_review`. Reviewer note in PR.md flags a *pre-existing* (not
  introduced here) inconsistency: `src/tck.rs::PINNED_TCK_SCENARIOS=1615`
  (definition count) is not wired to the harness `total` and must use the
  expanded count when a real shrinkage guard is wired in EPIC-002.
