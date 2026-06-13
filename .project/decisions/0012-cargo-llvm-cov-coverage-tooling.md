# 0012 — Coverage tooling: cargo-llvm-cov + grader-input wiring (T-0005)

- **Status:** decided (local/reversible; CI tooling, not a shipped dependency)
- **Date:** T+0:33 (2026-06-13)
- **Owner:** implementer-wf_84c0f0c7-752-17
- **Rubric refs:** Cat. 10 (tests/coverage/benches, GATE), Cat. 12 (process health)
- **Board item:** `.project/board/tasks/T-0005-ci-coverage-and-grader-inputs.md`

## Context

The rubric-grader scores Cat. 10 (and partly Cat. 4) from evidence emitted by CI.
Without a coverage number and a parseable summary block in the CI log, those
categories are capped at 25 ("asserted, unverified"). T-0005 wires the minimum
instrumentation: a coverage report, a coverage gate, a TCK results path, and a
`GRADER_INPUTS` summary block.

## Decision

1. **Coverage tool: `cargo-llvm-cov`.** Source-based (LLVM) coverage; the de-facto
   standard for Rust, accurate, and emits both LCOV and JSON. Added to the devenv
   shell (`flake.nix`) from `nixpkgs-unstable` alongside `cargo-nextest`, with the
   `llvm-tools-preview` toolchain component (which `cargo-llvm-cov` requires). In
   CI the GitHub-hosted route uses `dtolnay/rust-toolchain` + `taiki-e/install-action`.

2. **Grader glue is a tested shell script, not Rust.** `scripts/ci/grader-inputs.sh`
   assembles the `GRADER_INPUTS` block and enforces the coverage gate. It is
   test-first (`grader-inputs.test.sh`, 10 assertions) and re-run under
   `cargo nextest` via `tests/ci_grader_inputs.rs`. This keeps Rust code changes to
   the scaffold crate minimal while still gating the behaviour the grader depends on.

3. **Coverage gate starts at 0, ratchets to 90.** `COVERAGE_THRESHOLD` (CI env) is
   `0` initially so the near-empty crate is not blocked; raised toward the rubric's
   ≥90% as tests land. Policy documented in `docs/process/ci-grader-inputs.md`.

4. **TCK results path is the canonical `.project/reports/tck-latest.json`.** The
   board item suggested `tck-results-latest.json` *as an example* ("e.g."), but the
   master-rubric (Cat. 4, line 76), the `rubric-grader` agent, and the already-
   landed `test`-job TCK step all read/write `tck-latest.json`. Introducing a second
   path would silently diverge (the runner writes one, the grader reads the other),
   so the coverage job references the single canonical path and regenerates it via
   the `tck-runner`. No committed stub: the file is generated per run and archived as
   the `tck-results` artifact (a committed zero-count stub would shadow the live
   grader numbers). `grader-inputs.sh` degrades to `0/0` if the file is absent. This
   reconciliation is recorded so a future reader does not "fix" the path back to the
   board's example wording.

## License check (open-source guardrails §5)

- `cargo-llvm-cov`: **Apache-2.0 OR MIT** (verified via
  `nix eval .#legacyPackages...cargo-llvm-cov.meta.license` → `["Apache-2.0","MIT"]`).
  Both are in the approved family. It is a **dev/CI tool**, not a `Cargo.toml`
  dependency, so it does not enter the shipped binary's dependency graph.
- `taiki-e/install-action`, `actions/upload-artifact`, `Swatinem/rust-cache`,
  `dtolnay/rust-toolchain`: MIT/Apache GitHub Actions; CI-only.

## Alternatives considered

- **`cargo-tarpaulin`** — ptrace-based, Linux-only, less accurate on modern Rust;
  rejected (cargo-llvm-cov is more accurate and cross-platform).
- **Rust grader binary instead of a shell script** — heavier change to a scaffold
  crate for pure CI glue; rejected for now (the shell script is fully tested).

## Consequences

- The dev shell gains `cargo-llvm-cov` + `llvm-tools-preview`; `flake.lock`
  unchanged (same `nixpkgs-unstable` input already used for nextest).
- CI gains a `coverage` job that can fail the build on a coverage regression once
  the threshold is ratcheted. Reversible: drop the job / lower the threshold.
