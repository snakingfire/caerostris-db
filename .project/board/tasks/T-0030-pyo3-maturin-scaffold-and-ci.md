---
id: T-0030
title: PyO3 + maturin scaffold, importable package, pytest in CI
type: task
status: backlog
priority: P2
assignee:
epic: EPIC-007
deps: [T-0001]
rubric_refs: [8, 10]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

Cat. 8 (Python bindings). Stand up the PyO3 + maturin build so `maturin develop`
produces an importable package and `maturin build` produces a wheel, both running
in CI, with a pytest smoke test. This is the FFI substrate — it depends only on the
crate skeleton (T-0001); the real query/ingest API is wired once the engine exposes
it (T-0031). License-clean (PyO3/maturin are permissive). See `EPIC-007`.

## Acceptance criteria
- [ ] PyO3 + maturin wired: `maturin develop` produces an importable `caerostris` module; `maturin build` produces a wheel; both run in CI.
- [ ] A trivial exported function (e.g. version string) is callable from Python; a pytest smoke test asserts it.
- [ ] Rust panics surface as a typed Python exception, not a naked `RuntimeError` string (pattern established here).
- [ ] CI installs the toolchain, builds the wheel, and runs pytest; `ruff`/`flake8` clean for the Python test code.
- [ ] tests added (pytest smoke); coverage not regressed on the Rust side
- [ ] docs / ADR updated with the build + packaging instructions
- [ ] `./format_code.sh` green for the Rust side

## Notes / log
Ready now: depends only on T-0001. Establishes the FFI + exception-mapping pattern
that T-0031 builds the real API on.
