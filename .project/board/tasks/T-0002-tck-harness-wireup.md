---
id: T-0002
title: Wire openCypher TCK Gherkin runner and report live pass-rate
type: task
status: ready
priority: P0
assignee:
epic: EPIC-002
deps: []
rubric_refs: [4, 10]
estimate: M
created: T0
updated: T0
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

- [ ] TCK `.feature` files vendored or fetched by a reproducible script; pinned to a specific openCypher release; CI does not require external network access to run them.
- [ ] Gherkin parser integrated: all `.feature` files parsed without errors; scenario count matches the official TCK count for the pinned release.
- [ ] Harness runner executes each scenario against the engine adapter; unimplemented paths yield `pending`, not panics.
- [ ] Machine-readable output emitted: JSON (or structured text) with `total`, `pass`, `pending`, `fail`, `pass_rate` fields; file path documented so the rubric grader can find it.
- [ ] CI step added: TCK runner runs in CI; pass-rate is surfaced in the CI job summary or artifact.
- [ ] Initial run shows the correct `total` count matching the pinned TCK; zero unexpected `fail` results (all unimplemented = `pending`).
- [ ] Tests added for the harness itself: a synthetic `.feature` file with a trivially-passing scenario and a trivially-failing scenario, verifying the harness counts them correctly.
- [ ] `./format_code.sh` green.

## Notes / log

The engine adapter in this task is intentionally minimal (a stub that returns `pending` for every query). The language implementation comes in EPIC-002 stories that plug into this harness. Keep the harness/adapter interface clean so language implementors can fill it in independently.
