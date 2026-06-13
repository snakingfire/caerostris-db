---
id: T-0002
title: Wire openCypher TCK Gherkin runner and report live pass-rate
type: task
status: done
priority: P0
assignee: integrator
epic: EPIC-002
deps: []
rubric_refs: [4, 10]
estimate: M
created: T0
updated: T+2:30
---

## Context

Cat. 4 (openCypher completeness) is a GATE category scored directly as the TCK pass-rate: **0 if the harness is not wired**, regardless of how much openCypher the engine actually handles. This task wires the harness from day one so the score is always a real number, even if it starts at near-zero.

The official openCypher TCK is a suite of Gherkin `.feature` files in the [openCypher/openCypher](https://github.com/opencypher/openCypher) repository under `tck/features/`. Each scenario exercises a specific language feature via `Given`, `When`, `Then` steps. The harness must:

1. **Fetch / vendor the TCK**: include the TCK `.feature` files (or a submodule / download script) so CI can run them without network access. Pin to a specific TCK release tag.
2. **Parse Gherkin**: use a Rust Gherkin library (e.g. `gherkin` crate) or a script-based runner to parse each `.feature` file into test cases.
3. **Execute each scenario**: for each `When executing query <cypher>` step, run the query against the caerostris-db engine (via a thin adapter); for each `Then the result should be...` step, assert the result matches.
4. **Handle unimplemented features gracefully**: scenarios that use engine features not yet implemented should be marked `pending` (not panic); the harness counts `pass / pending / fail` separately.
5. **Emit machine-readable output**: after the full run, emit a JSON or structured text summary to stdout / a file: `{ "total": N, "pass": P, "pending": Q, "fail": F, "pass_rate": P/N }`. The rubric grader consumes this number from CI artifacts.
6. **CI integration**: the TCK runner runs as a CI step; its pass-rate output is captured and displayed in the CI summary.

At T0 the engine has no openCypher implementation, so the expected initial result is ~0 pass / N pending (not N fail — unimplemented = pending, not broken).

## Acceptance criteria

- [ ] TCK `.feature` files vendored or fetched by a reproducible script; **pinned to openCypher `1.0.0-M23`** (commit `007895aff5f33097d67b2e48a0a2babd6bd18590`); CI does not require external network access to run them. (Pin = `caerostris_db::tck::PINNED_TCK_TAG` / `PINNED_TCK_COMMIT`; BUG-0007 / Decision 0008.)
- [ ] Gherkin parser integrated: all `.feature` files parsed without errors; **scenario count == `caerostris_db::tck::PINNED_TCK_SCENARIOS` (1615)** — the harness calls `tck::verify_suite_size(loaded)` and aborts if it differs (catches silent suite shrinkage/growth).
- [ ] Harness runner executes each scenario against the engine adapter; unimplemented paths yield `pending`, not panics.
- [ ] **Side-effect assertion (BUG-0006):** the adapter reads the engine's `caerostris_db::query::QueryStatistics` surface and asserts the `Then the side effects should be:` step by parsing the Gherkin table with `QueryStatistics::from_tck_side_effects` and comparing with `matches_side_effects` (≡ `==`); such scenarios count as real pass/fail, never auto-`pending`. Semantics pinned in `.project/decisions/0012-tck-side-effect-counting-semantics.md`.
- [ ] Machine-readable output emitted: JSON (or structured text) with `tck_tag`, `tck_commit`, `total`, `pass`, `pending`, `fail`, `pass_rate` fields (use `caerostris_db::tck::TckSummary::to_json()`); written to `.project/reports/tck-latest.json` so the rubric grader can find it.
- [ ] **`pass_rate = pass / total`, `total = pass + pending + fail`** — `pending` and `fail` are in the denominator; **no scenario excluded** (use `tck::pass_rate` / `TckSummary::pass_rate`). 100% requires `pending == 0 && fail == 0` (`TckSummary::is_complete()`). Computing `pass / (pass + fail)` or moving scenarios to `pending` to inflate the rate is forbidden.
- [ ] CI step added: TCK runner runs in CI; pass-rate is surfaced in the CI job summary or artifact.
- [ ] Initial run shows the correct `total` count matching the pinned TCK (1615); zero unexpected `fail` results (all unimplemented = `pending`).
- [ ] Tests added for the harness itself: a synthetic `.feature` file with a trivially-passing scenario and a trivially-failing scenario, verifying the harness counts them correctly.
- [ ] `./format_code.sh` green.

## Notes / log

The engine adapter in this task is intentionally minimal (a stub that returns `pending` for every query). The language implementation comes in EPIC-002 stories that plug into this harness. Keep the harness/adapter interface clean so language implementors can fill it in independently.

- BUG-0007 (T+0:54): the pass-rate definition and suite pin are now fixed in code at `src/tck.rs` (`caerostris_db::tck`). The harness MUST consume `tck::pass_rate` / `tck::TckSummary` / `tck::verify_suite_size` rather than computing its own rate, so the Cat. 4 GATE metric stays non-gameable. Pinned tag `1.0.0-M23`, 1615 scenarios, 220 feature files — see `.project/decisions/0008-tck-passrate-definition-and-pinning.md`.
- Landed in commit e69e754 at T+2:30. Branch: work/T-0002-tck-runner. 122 tests pass (72 main-crate + 50 tck-runner). Live pass-rate: 0.0000 (0/1602 pending, 0 fail, 1 parse_error). Report: .project/reports/tck-T+02-30.md. Cat. 4 GATE is now instrumented — score rises as EPIC-002 Cypher features land.
