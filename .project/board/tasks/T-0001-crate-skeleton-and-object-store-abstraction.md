---
id: T-0001
title: Crate skeleton, workspace layout, and object-store abstraction with local mock
type: task
status: ready
priority: P0
assignee:
epic: EPIC-001
deps: [T-0000]
rubric_refs: [2, 12]
estimate: M
created: T0
updated: T0
---

## Context

This task sets up the Rust crate/workspace skeleton and the foundational `ObjectStore` abstraction that every other component will build on. It is intentionally **independent of the storage format spec** (SPIKE-0003) — it provides the substrate (trait + mock) that format writers and readers will use; the format decisions come later.

**Scope:**

1. **Workspace / module layout**: decide whether to stay as a single crate (per CLAUDE.md: single crate now, promote to workspace when the engine splits) or set up a minimal Cargo workspace. Add the module skeleton (`lib.rs`, `storage/`, `engine/`, `planner/`, `txn/`) with stub modules and `#[allow(dead_code)]` guards to keep clippy clean.

2. **`ObjectStore` trait**: a minimal async Rust trait (using `async_trait` or RPITIT) with at minimum: `put(key, bytes)`, `get(key) -> bytes`, `get_range(key, range) -> bytes`, `delete(key)`, `list(prefix) -> [key]`. The trait must be object-safe (or use `Arc<dyn ObjectStore>`) so it can be swapped between the real S3 client and the mock.

3. **Local S3 mock integration**: wire a MinIO-compatible test fixture (using the `aws-sdk-s3` crate against a local MinIO container, or `opendal`/`object_store` crate's in-memory/local backend) that the integration test suite can spin up. The fixture must be cheap (start/stop in seconds) and not require a running external service for unit tests — use the in-memory backend for unit tests, MinIO for integration tests.

4. **Smoke test**: a `#[tokio::test]` integration test that does:
   - starts the mock object store
   - `put`s a 1 KB object
   - `get`s it back
   - `get_range`s a sub-slice
   - `delete`s it
   - asserts the object is gone
   All assertions pass.

5. **CI configuration**: ensure `cargo build`, `cargo test`, and `./format_code.sh` all pass in CI with this skeleton in place.

This task intentionally does NOT implement any storage format, commit protocol, or graph logic — those come after their respective design spikes. The goal is a clean, compiling, tested substrate.

## Acceptance criteria

- [ ] `cargo build` and `cargo test` succeed on the skeleton with zero warnings (clippy `-D warnings` clean).
- [ ] `ObjectStore` trait defined: at minimum `put`, `get`, `get_range`, `delete`, `list` methods; documented with `///` doc comments.
- [ ] In-memory `ObjectStore` implementation for unit tests: zero external dependencies, deterministic.
- [ ] MinIO/S3-mock integration test fixture wired: a test helper that starts the mock and returns a configured `ObjectStore` handle; documented in `tests/README.md` or a code comment.
- [ ] Smoke test passes: put/get/get_range/delete round-trip against the mock backend.
- [ ] Module skeleton in place (`storage/`, `engine/`, `planner/`, `txn/`) with stub `mod.rs` files and `pub use` re-exports as appropriate; no dead-code warnings.
- [ ] `./format_code.sh` green (cargo fmt + clippy + taplo).
- [ ] CI config (`.github/workflows/` or equivalent) updated to run `cargo test` and `./format_code.sh`.

## Notes / log

Does not depend on SPIKE-0001 or SPIKE-0003 — deliberately. The format spec will slot into the storage module once ratified. Keep the `ObjectStore` trait minimal and stable; avoid baking format assumptions into the trait interface.
