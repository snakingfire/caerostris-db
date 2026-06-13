# Decision 0034 — TCK pin reconciliation: the vendored `2024.3` corpus is canonical

- **Date / marker:** T+4:30 (2026-06-13)
- **Owner:** test-author (BUG-0018, branch `work/BUG-0018-tck-parse-gap-citations-and-pin-reconcile`)
- **Status:** recorded; routes through steering (design-level: it amends the
  Cat. 4 GATE denominator definition recorded in Decision 0008 and the master
  rubric). Tracked by **BUG-0018** (P1).
- **Rubric:** Cat. 4 (openCypher/TCK, GATE), Cat. 12 (process health)
- **Relates to:** Decision 0008 (pass-rate definition + pinning), Decision 0013
  (Scenario-Outline expansion), BUG-0007, BUG-0009, BUG-0018.

## Context

Two *different* openCypher TCK pins coexisted on `main`, an inconsistency
BUG-0018 surfaced:

| Source | Tag | Commit | Denominator |
|--------|-----|--------|------------:|
| **Spec pin** — `src/tck.rs` (`caerostris_db::tck`), `master-rubric.md` Cat. 4, `testing-and-benchmarks.md` §6, Decision 0008 | `1.0.0-M23` | `007895a` | 1615 (definitions) |
| **Corpus pin** — vendored `tck/openCypher/` (`PINNED_TAG`/`PINNED_COMMIT`), the live `tck-runner` harness, Decision 0013, BUG-0009 | `2024.3` | `677cbaf` | 3884 (expanded cases) |

The live harness (`tck-runner`) parses the **vendored `2024.3` corpus** and emits
`total = 3884` to `.project/reports/tck-latest.json` — the number the
`rubric-grader` reads for the Cat. 4 GATE. The `caerostris_db::tck` contract
module (the BUG-0007 anti-gaming machinery: `PINNED_TCK_*`, `verify_suite_size`)
pins the **other** release (`1.0.0-M23` / 1615) and **is not wired into the live
harness at all** — `verify_suite_size` is exercised only by
`tests/tck_passrate_contract.rs` against literal constants, never against the
loaded corpus. So the suite-shrinkage guard guarded a pin that did not match the
suite actually being graded: a decorative integrity check.

## Finding

1. **The `1.0.0-M23` / 1615 spec pin is stale.** It was recorded at T0
   (Decision 0008) *before* the corpus was vendored. The corpus that was actually
   vendored, run, and BUG-0009-expanded is `2024.3` / `677cbaf`. `2024.3` is the
   newer, more complete openCypher TCK; re-vendoring back to `1.0.0-M23` would
   discard the vendored corpus and the BUG-0009 expansion work for an older suite
   — strictly worse for the "100% of openCypher" intent.
2. **`1615` is a definition count, `3884` is the executable-case count.** Per
   Decision 0013 / BUG-0009 the harness expands every `Scenario Outline` into one
   case per `Examples` row; `total` is now the expanded count. The canonical
   denominator the grader reads is therefore `3884`, not `1615`.
3. **The contract module was disconnected from the harness.** `verify_suite_size`
   guarded `1615` while the harness loaded `3884`; the guard would have *failed*
   had it ever been pointed at the real corpus.

## Decision

- **Canonical pin = the vendored corpus: `2024.3` / commit `677cbaf…`.** The spec
  pin is updated to match the corpus, not the other way around.
- **Canonical Cat. 4 denominator = `3884`** (the expanded executable test-case
  count the live harness reports as `total`). `1615` is retained only as the
  recorded scenario-*definition* count, for traceability.
- **Reconcile the spec artifacts to the canonical pin:**
  - `src/tck.rs`: `PINNED_TCK_TAG = "2024.3"`, `PINNED_TCK_COMMIT = "677cbaf…"`,
    `PINNED_TCK_SCENARIOS = 3884` (the expanded `total`; doc records the 1615
    definition count + the 13-scenario `Literals6` parse gap), feature files
    `220`. `verify_suite_size(loaded)` now checks against `3884` — i.e. against
    the count the live harness actually loads — closing the disconnect.
  - `master-rubric.md` Cat. 4 and `testing-and-benchmarks.md` §6: pin `2024.3` /
    `677cbaf`, denominator `3884` (definitions `1615`; expanded `3884`).
  - Decision 0008: superseded on the *specific numeric pin* by this decision
    (cross-linked); its pass-rate **definition** (`pass / total`, nothing
    excluded, 100 ⇒ `pending == 0 && fail == 0`) is unchanged and still binding.
- **The parse-error gap is named and owned by BUG-0018:** the single unparseable
  file is `expressions/literals/Literals6.feature` (13 scenarios in
  `parse_errors`, never `pending`/`fail`). It was previously *mis-cited* as
  "BUG-0008" (an unrelated SPDX license bug) in code/test/report comments; those
  citations are corrected to BUG-0018 in the same change.

## Alternatives considered

- **Re-vendor the corpus back to `1.0.0-M23` / 1615.** Rejected: discards the
  vendored `2024.3` corpus and the BUG-0009 expansion for an *older* suite;
  strictly worse for "100% of openCypher", and a much larger, riskier change than
  updating a stale pin to match reality.
- **Leave the two pins divergent and "document" it.** Rejected: a GATE
  integrity-guard that guards a non-loaded pin is a falsification of the
  anti-gaming machinery (Decision 0008 / BUG-0007). The commander's intent
  requires gaps be named honestly, not papered over.
- **Pin the definition count `1615` as the denominator.** Rejected: the grader
  reads the harness's `total`, which is the expanded `3884`; pinning `1615`
  re-opens the disconnect from the other side.

## Consequences

- The contract module and the live harness now agree on one pin (`2024.3` /
  `677cbaf` / `3884`); the suite-shrinkage guard is meaningful again.
- The Cat. 4 GATE bar is unchanged in *meaning* (100% ⇒ `pending == 0 &&
  fail == 0` over the whole pinned suite); only the pinned *identity* + recorded
  `total` move to the corpus that is actually graded.
- Bumping the pin later remains a deliberate, recorded action: update the
  `caerostris_db::tck` constants, the corpus `PINNED_*` markers, this decision,
  Cat. 4, and §6 in the same change, and re-measure `total`.

## Follow-ups (separate items, not blocking this reconciliation)

- **Wire `caerostris_db::tck::verify_suite_size` into the live `tck-runner`**
  (or `tck-runner/tests/vendored_corpus.rs`) so the guard runs against the loaded
  corpus, not only against literal constants. Filed as a note on BUG-0018; a
  dedicated `T-NNNN` may be split out by the planner.
- The grader instruction in `.claude/agents/rubric-grader.md` (L51–52) still
  references `1.0.0-M23` / `total == 1615` and must be updated to `2024.3` /
  `total == 3884` so the grader's tamper-check trusts the right pin. This edit is
  **deliberately deferred** here: modifying an agent-definition file is an
  agent-self-modification action outside the scope a test-author PR may take
  autonomously, and was blocked by the harness guardrail. It is flagged for an
  authorized follow-up (planner/steering or an `agent-config`-scoped change). The
  spec sources of truth (master-rubric Cat. 4, `testing-and-benchmarks.md` §6,
  `src/tck.rs`, Decision 0008 cross-link) ARE reconciled in this change, so the
  grader's *human-readable* anchor is correct; only the agent-prompt restatement
  lags.
