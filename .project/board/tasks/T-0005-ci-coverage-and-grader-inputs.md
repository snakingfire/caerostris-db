---
id: T-0005
title: Add cargo-llvm-cov coverage reporting and ensure CI emits grader-readable outputs
type: task
status: in_review
priority: P0
assignee: implementer-wf_156e2b80-bb6-3
epic: EPIC-009
deps: []
rubric_refs: [10, 12]
estimate: S
created: T0
updated: T+3:12
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
- [ ] TCK output path established: `.project/reports/tck-latest.json` (the canonical path the rubric grader reads — the board's `tck-results-latest.json` was an example "e.g."; reconciled to the canonical name in decision 0012) exists (regenerated per run by the `tck-runner`); CI archives it; its schema documented (fields: `total`, `pass`, `pending`, `fail`, `pass_rate`).
- [ ] `GRADER_INPUTS` summary block emitted in CI logs with all three fields (`coverage_pct`, `test_pass`, `tck_pass_rate`).
- [ ] CI artifact retention set to ≥7 days for coverage and TCK reports.
- [ ] `./format_code.sh` green; CI passes end-to-end after this change.

## Notes / log

No dep on T-0001 or T-0002 — this task can run in parallel or even before them. The coverage% will be 0 or N/A initially; that is fine. The infrastructure is what matters here, not the number.

- **T+0:40 (implementer-wf_84c0f0c7-752-17):** claimed, built TDD-first on branch
  `work/T-0005-add-cargo-llvm-cov-coverage-reporting-and-ensure-c`. Added
  `scripts/ci/grader-inputs.sh` (+ test harness `grader-inputs.test.sh`, 10
  assertions, run RED→GREEN) emitting the `GRADER_INPUTS` block + coverage gate;
  CI `coverage` job (cargo-llvm-cov LCOV+JSON, artifacts retention 14d);
  `flake.nix` adds cargo-llvm-cov (Apache-2.0/MIT) + llvm-tools-preview; TCK stub
  at `.project/reports/tck-results-latest.json`; docs `docs/process/ci-grader-inputs.md`;
  decision `0012`. Verified `cargo llvm-cov --json` locally in the devenv shell
  (line% = 60 via the CI jq query). `./format_code.sh` + `cargo nextest run` green;
  gitleaks clean. Status → `in_review`; PR at `.worktrees/T-0005/PR.md`. Pending
  adversarial-reviewer + premortem-analyst sign-off, then integrator land.

- **T+3:10 (implementer-wf_156e2b80-bb6-3):** re-took the in-flight branch (prior
  lane never landed; T-0005 still `ready` on `main`). **Rebased onto the latest
  `main`** — the branch's `ci.yml` was based on a stale `main` and the rebase
  cleanly merged the now-current jobs (openCypher TCK, gitleaks, cargo-deny,
  latency-sim) with the new `coverage` job, so no CI capability is regressed.
  **Reconciliations (drift fixes):** (1) reverted a foreign `BUG-0008` board edit
  that rode in on the rebase base; (2) **consolidated the TCK path onto the
  canonical `.project/reports/tck-latest.json`** (was `tck-results-latest.json`) —
  the rubric/grader/`test`-job all use the canonical name; a second path would
  silently diverge. Dropped the committed zero-count stub (it would shadow live
  grader numbers); the `coverage` job now regenerates the file via `tck-runner`
  and archives it. Updated `ci.yml`, `docs/process/ci-grader-inputs.md`, decision
  `0012`, and this item. `./format_code.sh` + tests green; gitleaks clean.
