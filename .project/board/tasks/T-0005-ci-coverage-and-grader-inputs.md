---
id: T-0005
title: Add cargo-llvm-cov coverage reporting and ensure CI emits grader-readable outputs
type: task
status: ready
priority: P0
assignee:
epic: EPIC-009
deps: []
rubric_refs: [10, 12]
estimate: S
created: T0
updated: T0
---

## Context

Cat. 10 (tests/coverage/benches, weight 8, GATE) is scored in part on the coverage percentage reported by cargo-llvm-cov in CI. Cat. 12 (process health) requires CI to stay green and emit machine-readable progress metrics the rubric grader can cite. Without this task, the grader cannot score Cat. 10 or Cat. 4 above 25 (the "asserted, unverified" floor) — no evidence = low score, regardless of actual coverage.

This task wires the minimum CI instrumentation needed for the grader to start producing evidence-based scores:

1. **cargo-llvm-cov**: add `cargo-llvm-cov` to the Nix devenv shell (`flake.nix`) and/or as a CI install step. Configure a `cargo llvm-cov --lcov` (or `--json`) run that generates a coverage report. The report must be saved as a CI artifact and its summary (line coverage%) emitted to stdout in a parseable format. The build must fail if line coverage drops below a configurable threshold (start at 0% to avoid immediately blocking the empty crate; ratchet up as coverage grows).

2. **TCK pass-rate output**: ensure the TCK runner from T-0002 (when it exists) emits its JSON summary to a known path (e.g. `.project/reports/tck-results-latest.json`) that CI archives as an artifact. If T-0002 is not yet complete, create the output path and a stub file so the grader knows where to look.

3. **CI summary step**: add a CI job step that prints a structured summary block:
   ```
   GRADER_INPUTS:
     coverage_pct: <N>
     test_pass: <pass>/<total>
     tck_pass_rate: <X>/<Y>
   ```
   This format is what the rubric grader's evidence-scraper parses from CI logs.

4. **Artifact retention**: configure CI to retain the coverage report (LCOV/HTML) and TCK results JSON for at least 7 days per run.

This task operates on CI configuration and `Cargo.toml` / `flake.nix` — minimal Rust code changes expected. It should be completable on the initial empty-crate skeleton from T-0001 (or before T-0001 if CI is set up independently).

## Acceptance criteria

- [ ] `cargo-llvm-cov` available in CI (installed via Nix shell or explicit CI install step); `cargo llvm-cov` runs without error on the codebase.
- [ ] Coverage report generated: LCOV file (or JSON) saved as a CI artifact; line coverage% emitted to stdout in the `GRADER_INPUTS` block.
- [ ] Coverage threshold configured in CI: build step that checks coverage% against a threshold (initially 0%; documented that it will be ratcheted to 90% as tests are added).
- [ ] TCK output path established: `.project/reports/tck-results-latest.json` exists (stub or real); CI archives it; its schema documented (fields: `total`, `pass`, `pending`, `fail`, `pass_rate`).
- [ ] `GRADER_INPUTS` summary block emitted in CI logs with all three fields (`coverage_pct`, `test_pass`, `tck_pass_rate`).
- [ ] CI artifact retention set to ≥7 days for coverage and TCK reports.
- [ ] `./format_code.sh` green; CI passes end-to-end after this change.

## Notes / log

No dep on T-0001 or T-0002 — this task can run in parallel or even before them. The coverage% will be 0 or N/A initially; that is fine. The infrastructure is what matters here, not the number.
