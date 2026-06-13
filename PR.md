# PR: BUG-0009 — TCK Scenario Outlines counted once, not expanded per Examples row

## Board item

[.project/board/tasks/BUG-0009-tck-scenario-outlines-counted-once-not-expanded-per-examples-row.md](.project/board/tasks/BUG-0009-tck-scenario-outlines-counted-once-not-expanded-per-examples-row.md)

Branch: `work/BUG-0009-outline-expansion` (based on the latest `main`, `494a9e7`).

## Rubric refs

Cat. 4 (openCypher / TCK pass-rate — GATE), Cat. 10 (tests / coverage — GATE)

## Acceptance criteria (from board item)

- [x] Harness expands each `Scenario Outline` into one executable scenario per
      `Examples` data row, substituting `<placeholder>` tokens into the query,
      setup statements, and expected-result cells, so `total` reflects the
      conventional openCypher test-case count (target: ~3880 at 2024.3, minus
      any still-unparseable files).
      → **3884** = 1326 parseable plain `Scenario:` + 2558 expanded outline
      cases (BUG-0008 `Literals6` still excluded as a `parse_error`).
- [x] Reconciliation guard test in `tck-runner/tests/vendored_corpus.rs`
      asserts the expanded count (so the denominator cannot silently shrink and
      certify a false 100%). → `outline_expansion_total_is_reconciled` pins
      `EXPANDED_TOTAL = 3884`, re-derives the composition from the authoritative
      `gherkin` parser, and additionally asserts **no `<placeholder>` survives
      expansion**.
- [x] (Interim) decision recorded and the gap named honestly — superseded:
      Decision 0013 is updated to **RESOLVED**, with the resolution note and the
      corrected counts.
- [x] `./format_code.sh` green.

## Summary of change

The `tck-runner` harness counted each `Scenario Outline` **once** rather than
expanding it into one executable scenario per `Examples` data row, and handed
the engine query/setup/result text with literal `<placeholder>` tokens still in
it. This made the Cat. 4 (GATE) denominator ~2.4× too small (1602 vs the
conventional ~3880 at tag `2024.3`) — the exact curated-subset gaming Decision
0008 forbids — and was a latent false-`fail` / stuck-`pending` defect that would
activate the moment a real engine (EPIC-002) plugged in.

This change adds `tck_runner::outline::expand_scenario`: a plain `Scenario:`
yields itself; a `Scenario Outline:` yields one concrete scenario per `Examples`
data row across every `Examples:` block, substituting each `<header>` token into
the scenario name, every step value, every docstring (the query + `Given having
executed:` setup), and every data-table cell (substitution is a literal textual
replace, matching the Gherkin contract, so a placeholder inside a larger literal
like `'<text>'` substitutes correctly). A header-only outline (no data rows)
yields nothing — it has zero runnable test cases. `runner::all_scenarios` now
expands before `classify`, so `Summary::total` is the expanded test-case count.

While implementing this I found and corrected a latent bug in the **old** guard's
`grep`-style row counter: it dropped 17 commented-out (`#| ... |`) `Examples`
rows in `expressions/precedence/Precedence1.feature` (reporting 36 of the 53 the
`gherkin` parser actually expands). The parser is authoritative — it is what
drives the engine — so the documented `EXAMPLES_DATA_ROWS` is corrected from
2541 to **2558** and the new guard derives counts from the parser.

See `.project/decisions/0013-tck-scenario-outline-expansion-gap.md` (now RESOLVED).

## Test evidence

`cargo nextest run -p tck-runner` — **61 passed, 0 skipped** (was 50 on `main`;
+11 new). New tests:
- `outline::tests::*` (9): plain passthrough, expand-per-row, substitution in
  query/result-cell/name, no-examples-survive, embedded-literal, multi-block,
  header-only-yields-nothing.
- `runner::tests::outline_is_expanded_into_one_scenario_per_examples_row`,
  `runner::tests::expanded_scenarios_run_substituted_queries_and_pass` (a
  scripted engine that only answers *substituted* queries, proving the engine
  never sees a literal `<value>`).
- `vendored_corpus::outline_expansion_total_is_reconciled` (rewritten) +
  `corpus_expands_to_expected_total` + `report_json_carries_counts_and_provenance`
  (updated to the expanded `3884`).

`cargo nextest run --workspace --all-features` — **184 passed, 0 skipped**.
`cargo test -p tck-runner --doc` — 1 passed.

Live corpus run (stub engine) confirms the expanded denominator:

```
$ cargo run -q -p tck-runner -- --format json
{ "total": 3884, "pass": 0, "pending": 3884, "fail": 0, "parse_errors": 1, "pass_rate": 0.0,
  "tck_tag": "2024.3", "pinned_commit": "677cbafabb8c3c5eed458fd3b1ec0daec8d67d23" }
```

`./format_code.sh` — green (cargo fmt --all; clippy --workspace --all-targets
--all-features -D warnings; latency-sim sub-workspace; taplo).

Coverage: `cargo-llvm-cov` is not installed in this sandbox (no rustup /
llvm-tools-preview; CI runs it). The new `outline.rs` is small and every branch
is exercised by the unit tests above; the runner/corpus integration tests drive
the real path end-to-end, so coverage of the new code is effectively complete
and Cat. 10 is not regressed.

## Environment

`scripts/env/up.sh` (shared S3 mock already up) + `scripts/env/bucket.sh BUG-0009`
were run for an isolated bucket/prefix. This change does not touch the S3 code
path (it is harness-only), so no integration test against the mock is required by
this diff; the workspace suite is green.

## Reviewer note — pre-existing inconsistency to track (not introduced here)

`caerostris_db::tck::PINNED_TCK_SCENARIOS = 1615` (in `src/tck.rs`) and its
`verify_suite_size()` guard pin the scenario-**definition** count (1615), which
is a different quantity from the harness's expanded test-**case** `total` (3884).
`verify_suite_size` is **not** currently wired to the harness `total` (it is only
exercised by `tests/tck_passrate_contract.rs` with synthetic inputs), so this
change does not trip it. When EPIC-002 wires a real shrinkage guard against the
harness `total`, it must use the expanded count (or a separate expanded pin), not
the 1615 definition pin. The real shrinkage protection for the harness today is
`vendored_corpus::outline_expansion_total_is_reconciled`.

## Review gate

- [x] adversarial-reviewer sign-off (see docs/process/adversarial-review-loops.md)
- [ ] premortem-analyst sign-off (see docs/process/adversarial-review-loops.md)
- [x] `./format_code.sh` green
- [x] `cargo nextest run` green (or `cargo test` outside Nix shell)
- [ ] coverage not regressed
- [ ] board item updated to `in_review`

<!-- Reviewers: append your verdict block below this line per adversarial-review-loops.md -->

## Adversarial Review

**Verdict:** approve

**Blocking findings** (must be fixed before landing):
- none.

**Non-blocking observations** (consider in a follow-up):
- The reconciliation guard's `scenario_has_placeholder` (in `vendored_corpus.rs`)
  scans step **docstrings** and **data-table cells** for surviving `<placeholder>`
  tokens, but not `step.value`. `substitute_step` *does* substitute `step.value`
  (so production is correct), and I confirmed no outline in the pinned corpus
  carries a placeholder in a `When`/`Then` step value line (`grep -rE '^\s*(When|Then|And)
  .*<[a-zA-Z_]+>'` returns nothing), so no false 100% is reachable through this gap
  today. Adding `step.value` to the survivor scan would future-proof the guard
  against a corpus that moves placeholders onto the step line.
- Branch merge-base is `494a9e7`; `main` is now `cf70365` (12 commits ahead). None
  of those 12 commits touch `tck-runner/` (`git diff --name-only 494a9e7..main`
  shows no tck-runner paths), so the land-time rebase should be clean. The only
  shared-file overlap risk is the board item and Decision 0013, which are not in
  the main delta. Integrator should rebase before landing per the PR workflow.
- The PR's own reviewer note correctly flags the pre-existing inconsistency between
  `caerostris_db::tck::PINNED_TCK_SCENARIOS = 1615` (scenario-*definition* count)
  and the harness's expanded test-*case* `total = 3884`. I confirmed `verify_suite_size`
  is only ever called from `src/tck.rs` and `tests/tck_passrate_contract.rs` with
  synthetic inputs — it is **not** wired to the runtime `total` — so this change
  does not trip it. Not introduced here; track for EPIC-002's real shrinkage guard.

**Attacks attempted and survived** (mandatory):
- **Sequential-substitution / prefix-overlap corruption** (`substitute` does ordered
  `String::replace`): the corpus has prefix-overlapping placeholder names
  (`<map>` vs `<map2>` in `Temporal7.feature`). Survived — the `<…>` delimiters mean
  `<map>` is not a substring of `<map2>`, so no spurious replacement. I also checked
  every Examples block for a data cell that contains a *header* `<token>`
  (the only second-order double-substitution vector): **0 found** across all 220
  feature files. `outline_expansion_total_is_reconciled` independently confirms **0**
  surviving placeholders over the whole corpus.
- **Denominator gaming (Decision 0008 / Cat. 4 GATE):** survived. `Summary::record`
  only counts runnable scenarios into `total`; `parse_errors` (the 13 Literals6
  scenarios, BUG-0008) stay excluded. Expansion can only *grow* the denominator
  (1602 → 3884), and reaching Cat. 4 = 100 now genuinely requires every Examples
  variant to pass — the exact anti-gaming intent of BUG-0009. Strict improvement.
- **Header-only outline mis-count** (the "yields nothing" branch): survived. I found
  exactly one suspected header-only Examples block (`Precedence1.feature`) and
  confirmed it is actually a *commented-out* (`#| = |`) data row that the `gherkin`
  parser correctly skips to read `| <= |` — i.e. the very `#|`-comment case that
  drove the 2541 → 2558 grep-vs-parser correction. **Zero genuine header-only
  outlines** exist, so the count (3884) is unaffected by that decision.
- **Build / gate verification:** ran `./format_code.sh` → exit 0 (fmt + clippy
  `-D warnings` + taplo, all green); `cargo test --workspace --all-features` →
  all suites pass, 0 failures, 0 warnings; `cargo run -p tck-runner -- --format json`
  → live `total: 3884, pass: 0, pending: 3884, fail: 0, parse_errors: 1`, matching
  the PR's claim; `vendored_corpus` integration tests (incl. the reconciliation
  guard) pass against the real pinned `2024.3` corpus (220 files).
- **Scope / blast radius:** survived. Unlike the abandoned sibling branches
  (284 files / +45k lines), this branch is correctly re-cut from a clean base: 8
  files, all `tck-runner`-scoped + the board/decision docs. No `unsafe`, no secrets,
  no new dependency (`gherkin` was already present).

**Rationale:** The fix is correct and minimal: `expand_scenario` substitutes every
`<placeholder>` surface the harness actually reads (query/setup docstrings, step
values, result/side-effect table cells), `runner::all_scenarios` expands before
classifying so `total` reflects the conventional openCypher test-case count, and a
reconciliation guard pins the expanded denominator and asserts zero surviving
placeholders against the real corpus. It strengthens the Decision 0008 GATE integrity
(denominator can only grow; 100 now means all Examples variants) rather than weakening
it. All gates are green and every attack I constructed failed to land. The remaining
items are non-blocking. (Note: the integrator must still obtain `premortem-analyst`
sign-off and rebase onto current `main` before landing — both review-gate boxes must
be checked.)

**Signed:** adversarial-reviewer  T+3:2x
