# CI Grader Inputs — coverage, tests, TCK

> How CI emits the machine-readable signals the `rubric-grader` cites as
> evidence for **Cat. 10** (tests/coverage/benches) and **Cat. 4** (openCypher
> TCK pass-rate), and how the coverage gate works. Wired by `T-0005`.

The rubric is **evidence-based**: a score claim must cite an artifact (a passing
test, a coverage number, a TCK pass-rate). No evidence → the category is capped at
25 ("asserted, unverified"). This doc describes the artifacts CI produces so the
grader can score those categories above the floor.

## The `GRADER_INPUTS` block

Every CI run prints one structured block to the log (and to the GitHub step
summary). Its shape is fixed — the grader's scraper matches on it verbatim:

```
GRADER_INPUTS:
  coverage_pct: <N>          # line coverage %, from cargo-llvm-cov JSON
  test_pass: <pass>/<total>  # from the libtest summary lines
  tck_pass_rate: <X>/<Y>     # from .project/reports/tck-latest.json
```

It is produced by [`scripts/ci/grader-inputs.sh`](../../scripts/ci/grader-inputs.sh),
which is unit-tested by `scripts/ci/grader-inputs.test.sh` and exercised under
`cargo nextest run` via `tests/ci_grader_inputs.rs`. The script also enforces the
coverage gate (below), so the block and the gate never drift apart.

## Coverage: `cargo-llvm-cov`

The `coverage` CI job runs `cargo llvm-cov` and emits two report shapes from a
single instrumented build:

- `coverage/lcov.info` — LCOV (uploaded as the `coverage-report` artifact;
  Codecov-ready).
- `coverage/coverage.json` — machine-readable; the line% is read with
  `jq -r '.data[0].totals.lines.percent'` and fed to the `GRADER_INPUTS` block.

Locally: `cargo llvm-cov --summary-only` (the dev shell provides `cargo-llvm-cov`
and the `llvm-tools-preview` toolchain component via `flake.nix`).

### The coverage gate (the ratchet)

`grader-inputs.sh --coverage <pct> --threshold <pct>` exits non-zero when
`coverage < threshold`, failing the build. The threshold is the CI env var
`COVERAGE_THRESHOLD`, **initially `0`** so the early/empty crate is never blocked.

**Ratchet policy:** raise `COVERAGE_THRESHOLD` as real tests land — the rubric
target is **≥90%** (Cat. 10). Bump it in step with measured coverage so a
regression below the established line fails CI. Never lower it to make a red build
green; report the honest gap instead.

## TCK results: `.project/reports/tck-latest.json`

This is the **one canonical path** the rubric grader reads for the Cat. 4 TCK
pass-rate (master-rubric Cat. 4). The openCypher TCK runner
(`caerostris_db`/`tck-runner`, wired by `T-0002`) writes its summary here. Both
the `test` job and the `coverage` job regenerate it (each on its own runner) and
archive it as a CI artifact, so the grader always has a well-formed, current file
to read. The file itself is generated, never committed (it would otherwise shadow
the live numbers); see `.gitignore`.

> **Path note (drift avoidance):** the T-0005 board item suggested the name
> `tck-results-latest.json` as an example. We use the **canonical**
> `tck-latest.json` instead, because that is the exact path the master-rubric
> (Cat. 4, line 76), the `rubric-grader` agent, and the existing `test`-job TCK
> step already read/write. A second path would silently diverge (the runner
> writes one, the grader reads the other). One path, no drift.

### Schema

The runner emits `caerostris_db::tck::TckSummary::to_json()`; the fields the
grader and `grader-inputs.sh` depend on are:

| Field        | Type    | Meaning                                             |
|--------------|---------|-----------------------------------------------------|
| `total`      | integer | Total TCK scenarios considered (`pass+pending+fail`).|
| `pass`       | integer | Scenarios passing.                                  |
| `pending`    | integer | Scenarios not yet implemented.                      |
| `fail`       | integer | Scenarios failing (wrong result).                   |
| `pass_rate`  | number  | `pass / total` in `[0.0, 1.0]` (0.0 when `total=0`).|
| `tck_tag`    | string  | Pinned openCypher release tag (suite-shrinkage guard).|
| `tck_commit` | string  | Pinned upstream commit the corpus was vendored from.|

`grader-inputs.sh` reads `pass` and `total` from this file (degrading to `0/0` if
the file is absent or unparseable) and emits `tck_pass_rate: <pass>/<total>`.

## Artifact retention

Both the coverage report and the TCK results are uploaded with
`retention-days: 14` (≥ the 7-day requirement), so the grader can fetch the most
recent run's evidence.
