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
  tck_pass_rate: <X>/<Y>     # from .project/reports/tck-results-latest.json
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

## TCK results: `.project/reports/tck-results-latest.json`

The openCypher TCK runner (`T-0002`) writes its summary here; CI archives it as
the `tck-results` artifact and the grader reads the pass-rate from it. Until the
runner lands, a schema-versioned **stub** (all-zero counts) occupies the path so
the grader and CI always have a well-formed file to read.

### Schema

| Field            | Type    | Meaning                                            |
|------------------|---------|----------------------------------------------------|
| `schema_version` | integer | Schema version (currently `1`).                    |
| `generated_at`   | string  | When the run was produced (`T+` marker or ISO).    |
| `harness`        | string  | `"stub"` or the runner name once `T-0002` lands.   |
| `total`          | integer | Total TCK scenarios considered.                    |
| `pass`           | integer | Scenarios passing.                                 |
| `pending`        | integer | Scenarios not yet implemented / skipped.           |
| `fail`           | integer | Scenarios failing.                                 |
| `pass_rate`      | number  | `pass / total` in `[0.0, 1.0]` (0.0 when `total=0`).|
| `note`           | string  | Free-text context (optional).                      |

`grader-inputs.sh` reads `pass` and `total` from this file (degrading to `0/0` if
the file is absent or unparseable) and emits `tck_pass_rate: <pass>/<total>`.

## Artifact retention

Both the coverage report and the TCK results are uploaded with
`retention-days: 14` (≥ the 7-day requirement), so the grader can fetch the most
recent run's evidence.
