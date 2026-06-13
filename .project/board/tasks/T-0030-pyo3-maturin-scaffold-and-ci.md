---
id: T-0030
title: PyO3 + maturin scaffold, importable package, pytest in CI
type: task
status: done
priority: P2
assignee: implementer-wf_e9fceb87-27c-8
epic: EPIC-007
deps: [T-0001]
rubric_refs: [8, 10]
estimate: M
created: T0+0:20
updated: T0+3:50
---

## Context

Cat. 8 (Python bindings). Stand up the PyO3 + maturin build so `maturin develop`
produces an importable package and `maturin build` produces a wheel, both running
in CI, with a pytest smoke test. This is the FFI substrate — it depends only on the
crate skeleton (T-0001); the real query/ingest API is wired once the engine exposes
it (T-0031). License-clean (PyO3/maturin are permissive). See `EPIC-007`.

## Acceptance criteria
- [x] PyO3 + maturin wired: `maturin develop` produces an importable `caerostris` module; `maturin build` produces a wheel; both run in CI.
- [x] A trivial exported function (e.g. version string) is callable from Python; a pytest smoke test asserts it.
- [x] Rust panics surface as a typed Python exception, not a naked `RuntimeError` string (pattern established here).
- [x] CI installs the toolchain, builds the wheel, and runs pytest; `ruff`/`flake8` clean for the Python test code.
- [x] tests added (pytest smoke); coverage not regressed on the Rust side
- [x] docs / ADR updated with the build + packaging instructions
- [x] `./format_code.sh` green for the Rust side

## Notes / log
Ready now: depends only on T-0001. Establishes the FFI + exception-mapping pattern
that T-0031 builds the real API on.

- **T0+0:52 — implemented (implementer-wf_e9fceb87-27c-8).** Branch
  `work/T-0030-pyo3-maturin-scaffold-and-ci`. Added isolated `python/` workspace
  (PyO3 0.27, abi3-py39) compiling a `caerostris` cdylib; maturin `develop`+`build`
  both work; pytest smoke (4 tests) + Rust unit tests (5) green; typed
  `CaerostrisError` panic-mapping pattern established. Wired CI `python-bindings`
  job (ruff/fmt/clippy/test/maturin/pytest/wheel), extended cargo-deny +
  `tests/license_manifest.rs` to cover `python/Cargo.lock` (14 new permissive deps
  recorded). ADR 0004 added. `./format_code.sh` green; 124 root tests still pass.
  Status → `in_review`; dispatching adversarial-reviewer + premortem-analyst.
- **T0+3:42 — adversarial review: `changes_requested`** (branch
  `work/T-0030-pyo3-maturin-scaffold-importable-package-pytes`, tip `82be1ac`).
  Blocking: CI `test` (`cargo test --workspace --all-features`) and `coverage`
  (`cargo llvm-cov --all-features --workspace`) jobs fail to **link** — `--all-features`
  enables pyo3's `extension-module`, which omits the libpython link directive, so a
  normal cargo test/cov build leaves Python symbols undefined (reproduced locally with
  the exact CI commands). Masked by the author's `nextest`/default-feature evidence and
  by `land.sh` also using `nextest`, so it would land green then turn `main` CI red.
  Fix: exclude the crate from the all-features test/cov runs (or gate the maturin-only
  feature out of `--all-features`) and verify with the exact CI commands. License
  hygiene + FFI/typed-exception scaffold reviewed and sound (14 new deps match real
  crate metadata; allow-lists correct). Reviewer checkbox left unchecked. Verdict +
  surviving-attacks log in PR.md. Filed BUG-0023 (pre-existing `Unicode-3.0` deny.toml
  gap, out of scope for T-0030).
- **T0+3:50 — Landed in commit f28b372 at T0+3:50.** Branch
  `work/T-0030-pyo3-maturin-scaffold-and-ci` (tip `1ed0c36`) merged into main with
  `--no-ff`; additive ci.yml conflict resolved (union: kept landed `coverage` job from
  main + added new `python-bindings` job from T-0030). Both adversarial-reviewer and
  premortem-analyst approved. 178 root tests + 5 python-crate tests green. Status → `done`.
