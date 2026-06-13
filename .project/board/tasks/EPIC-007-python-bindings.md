---
id: EPIC-007
title: Python embedded bindings
type: epic
status: backlog
priority: P1
assignee:
epic:
deps: []
rubric_refs: [8]
estimate: L
created: T0
updated: T0
---

## Context

caerostris-db must expose a Pythonic embedded API (Cat. 8, weight 6) over the Rust engine — open/attach (all four modes from R3), run parameterized openCypher queries, ingest data, and return results as native Python objects. This is the primary non-Rust interface and makes the database usable from the Python data-science / analytics ecosystem without a separate server process.

Implementation approach: **PyO3 + maturin** (idiomatic Rust-to-Python FFI). The Python package must be buildable from source (`maturin build`), tested with pytest in CI (integration tests against the local S3 mock), and cover the full session lifecycle. Results must come back as Python dicts/lists/primitives — not opaque Rust objects — so downstream Python code can use them without further ceremony.

The bindings wrap the Rust engine's public API; they do not re-implement logic. Any gap in the Rust API needed to drive the Python layer cleanly should be filed as a task against EPIC-001 or EPIC-002.

Relevant requirements: R8 (Python bindings), R3 (all attach modes accessible from Python).

## Acceptance criteria

- [ ] PyO3 + maturin build wired: `maturin develop` produces an importable Python package; `maturin build` produces a wheel; both run in CI.
- [ ] `open`/`attach` API: Python code can open a database in each of the four attach modes (writer-master, read-only, master-less, via-server).
- [ ] Parameterized openCypher queries: Python code can run `db.query("MATCH (n) WHERE n.id = $id RETURN n", id=42)` and receive results.
- [ ] Ingest API: Python code can insert nodes and edges (or bulk-load from a dict/list structure) and query them back.
- [ ] Results as native Python objects: query results are Python lists of dicts; node/edge properties come back as Python ints, floats, strings, booleans, lists, dicts — not opaque handles.
- [ ] pytest suite in CI: at least one integration test per attach mode and one ingest + query round-trip test, all run against the local S3 mock.
- [ ] Error handling: Rust panics and domain errors surface as typed Python exceptions (not naked `RuntimeError` strings).
- [ ] `./format_code.sh` green for the Rust side; `ruff` or `flake8` clean for Python test code.

## Notes / log

Depends on EPIC-001 (storage abstraction) and EPIC-002 (query engine) being sufficiently complete to expose a usable Rust API. Can be started as soon as a minimal query round-trip is possible — full TCK completion is not a prerequisite.

**Remote (`via-server`) attach mode protocol:** the Python remote-read client targets the server-mode protocol decided in **ADR 0003** (`docs/adr/0003-server-mode-network-protocol.md`, SPIKE-0009): **gRPC over HTTP/2** via the shared `.proto`. A thin generated `grpcio`/`betterproto` stub gives the remote-read client without hand-written socket code; the typed `Value` oneof returns native Python objects. The embedded modes use the Rust engine directly (PyO3), not gRPC.
