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

- [ ] adversarial-reviewer sign-off (see docs/process/adversarial-review-loops.md)
- [ ] premortem-analyst sign-off (see docs/process/adversarial-review-loops.md)
- [ ] `./format_code.sh` green
- [ ] `cargo nextest run` green (or `cargo test` outside Nix shell)
- [ ] coverage not regressed
- [ ] board item updated to `in_review`

<!-- Reviewers: append your verdict block below this line per adversarial-review-loops.md -->
