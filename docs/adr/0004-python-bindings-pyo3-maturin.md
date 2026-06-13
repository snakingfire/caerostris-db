# ADR 0004 — Python bindings: PyO3 + maturin scaffold and packaging

## Status

`accepted`

<!-- This is an implementation-tooling ADR for an already-ready task (T-0030,
     EPIC-007). It records the build/packaging approach chosen and implemented;
     it does not change engine architecture, so it does not gate on steering
     pre-ratification of a design (it still passes the normal adversarial +
     pre-mortem code-review gate as part of the T-0030 PR). -->

## Date / T+ marker

T0+0:42 (2026-06-13)

## Context

The master rubric Cat. 8 (Python embedded bindings, weight 6) requires a
Pythonic embedded API over the engine, "packaged + tested (pytest) in CI". The
swarm's EPIC-007 fixes the FFI approach as **PyO3 + maturin**. T-0030 is the
scaffold task: it stands up the build so an importable module and a wheel are
produced and exercised in CI, and establishes the **error-mapping pattern** that
the real query/ingest API (T-0031) builds on. It depends only on the crate
skeleton (T-0001).

Hard constraints that bound the choice:

- **The engine binary and its cold-start path must not absorb PyO3.** Cat. 3's
  latency theorem must hold without anything the bindings drag in. PyO3 pulls a
  build-time tree (`pyo3-*`, `libc`, `indoc`, `target-lexicon`, …) that has no
  business in the engine's dependency graph or `Cargo.lock`.
- **License-clean only** (open-source-guardrails §5). Every crate in the PyO3
  tree must be recorded and permissive.
- **`./format_code.sh` green; clippy warnings are errors.** A naïve PyO3 crate
  in the root workspace breaks `cargo test` (the `extension-module` feature
  leaves libpython symbols unresolved, so a plain test binary fails to link).

## Decision

We will build the Python bindings as a **separate, isolated Cargo workspace at
`python/`** (own `Cargo.lock`, empty `[workspace]` table — the same pattern as
`formal/latency-sim`), compiling a `cdylib` named `caerostris` that **maturin**
packages as an abi3 (`abi3-py39`) extension module.

- **PyO3 ≥ 0.27**, `abi3-py39` for a single forward-compatible wheel (no
  per-interpreter rebuild). The `extension-module` feature is **opt-in**
  (declared as the crate's own feature, enabled by maturin via
  `[tool.maturin] features`), NOT a default, so `cargo test` links libpython and
  runs the crate's unit tests; an `auto-initialize` dev-dependency feature gives
  those tests an embedded interpreter.
- **Error mapping:** all engine failures surface to Python as a dedicated
  `caerostris.CaerostrisError` exception (created with `create_exception!`),
  never a naked `RuntimeError` string. A `map_panic_to_exception` helper wraps
  fallible calls with `catch_unwind` and converts panic payloads to that typed
  exception. T-0031 routes its query/ingest entry points through this.
- **CI:** a `python-bindings` job installs Rust + Python + maturin/pytest/ruff,
  runs `ruff check`/`ruff format --check`, fmt/clippy/test on the crate,
  `maturin develop` + `pytest`, and `maturin build` (wheel uploaded as an
  artifact). The license-check job additionally runs `cargo deny` against
  `python/Cargo.toml`, and `tests/license_manifest.rs` audits `python/Cargo.lock`
  against the shared manifest so a new PyO3-tree crate cannot land unrecorded.

## Alternatives considered

### Alternative A — PyO3 crate as a member of the root workspace

**Description:** Add `python` to the root `[workspace] members` so one
`Cargo.lock` covers everything.

**Why considered:** One lockfile, one `cargo build --workspace`, simplest mental
model; the existing `cargo deny` and `license_manifest.rs` would cover it for
free.

**Why rejected:** It forces PyO3's entire build tree into the **engine's**
`Cargo.lock` and license graph, contaminating the crate whose cold-start path
Cat. 3 must reason about. It also makes `cargo test --workspace`/`clippy
--workspace` try to build the `extension-module` cdylib, which fails to link
without an interpreter. Isolating the workspace (as `formal/latency-sim` already
does) avoids both.

### Alternative B — `cffi` / hand-written C ABI over a `#[no_mangle]` extern "C" surface

**Description:** Expose a C ABI from the engine crate and bind it from Python
with `cffi`/`ctypes`.

**Why considered:** No PyO3 dependency at all; a pure-C boundary is
language-agnostic and could serve other FFI consumers.

**Why rejected:** It throws away PyO3's automatic GIL handling, native-type
conversion (Cat. 8 requires results as native Python objects, not handles), and
exception mapping — all of which we would re-implement by hand and get subtly
wrong. EPIC-007 already fixed PyO3 + maturin as the approach; this ADR records
the *how*, not a re-litigation of *whether*.

## Consequences

### Positive

- Advances Cat. 8: an importable `caerostris` module and a wheel, both built and
  tested (pytest) in CI — the scaffold the real API (T-0031) drops onto.
- Keeps PyO3 entirely out of the engine crate's lockfile, license graph, and
  cold-start path (protects Cat. 3 reasoning).
- Establishes the typed-exception pattern (`CaerostrisError`) up front, so the
  query/ingest API never regresses to opaque `RuntimeError` strings.
- The license guard (`tests/license_manifest.rs` + `cargo deny`) now covers the
  PyO3 workspace too — Cat. 12 stays honest as the tree grows.

### Negative / trade-offs

- A second `Cargo.lock` and a second `cargo deny`/fmt/clippy invocation to keep
  in sync; `format_code.sh` and CI carry explicit `--manifest-path python/...`
  steps (mirrors the existing latency-sim handling, so the pattern is familiar).
- abi3 forgoes a few version-specific PyO3 fast paths; acceptable for a binding
  layer (correctness and one-wheel simplicity outweigh micro-perf here).
- The scaffold deliberately exports only `version()` + the error type; the real
  open/attach/query/ingest API is explicitly deferred to T-0031.

### Open questions

- The four attach modes and parameterized-query/ingest surface (EPIC-007
  acceptance criteria) are T-0031, once the engine exposes a usable Rust API.
- S3-mock integration tests for the Python layer arrive with T-0031 (this
  scaffold's smoke test needs no S3).

## Rubric impact

| Cat. | Name | Impact |
|------|------|--------|
| 8 | Python embedded bindings | Scaffolds the PyO3+maturin build, importable module, wheel, pytest in CI; moves Cat. 8 off the floor. |
| 10 | Tests, coverage & benchmarks | Adds a pytest smoke suite + Rust unit tests for the bindings crate to CI. |
| 12 | Engineering & process health | Keeps the license manifest/`cargo deny` complete across the new isolated workspace. |

## Sign-off

### Adversarial review record

_(no rounds yet — see the T-0030 PR)_

### Steering ratification

_(tooling ADR for a ready task; ratified via the T-0030 code-review gate)_
