---
id: T-0048
title: S3-compatible ObjectStore adapter + re-run manifest integration on the mock
type: task
status: backlog
priority: P2
assignee:
epic: EPIC-001
deps: []
rubric_refs: [2, 1, 10]
estimate: M
created: T0+3:55
updated: T0+3:55
---

## Context

The storage layer only ships the in-process `MemoryStore` backend today
(`src/storage/memory.rs`). The integration suite for the commit/manifest path
(`tests/manifest_resolution.rs`, landed with **T-0009**) therefore exercises the
manifest logic through the `ObjectStore` trait against `MemoryStore` — correct
and backend-agnostic, but **not** yet against the local S3 mock the rubric
(Cat. 2 / Cat. 10) and `docs/process/testing-and-benchmarks.md` §3 call for.

This task lands an S3-compatible `ObjectStore` adapter (against the self-
provisioned MinIO/moto mock; env contract in
`docs/process/parallel-execution-and-environment.md`) and re-runs the existing
manifest integration test against it **unchanged** — the manifest module was
written to depend only on the `ObjectStore` contract precisely so this swap is
free. It also discharges ADR 0002 §3's **mock-fidelity obligation** (two
concurrent `PUT If-None-Match:*` → exactly one `200` / one `412`), which the
commit task (T-0010) consumes.

License note: an S3 client crate (e.g. `object_store`, `aws-sdk-s3`, or `rusty-s3`)
is a **new dependency** — license-check via `cargo deny check licenses` and record
it in the license manifest **before** adding (guardrails §5).

## Acceptance criteria
- [ ] An `ObjectStore` impl backed by an S3-compatible client, wired to the
      `CAEROSTRIS_S3_*` env contract; integration tests **skip gracefully** when
      `CAEROSTRIS_S3_ENDPOINT` is unset (per testing-and-benchmarks §3).
- [ ] `tests/manifest_resolution.rs` is parameterised to run against the S3 mock
      (not only `MemoryStore`) when the endpoint is set — same assertions, real
      backend; uses `scripts/env/up.sh` + `scripts/env/bucket.sh` for isolation.
- [ ] ADR 0002 §3 mock-fidelity test: two concurrent create-only PUTs to one
      manifest key → exactly one success, one precondition failure.
- [ ] new S3 client dependency license-checked (`cargo deny check licenses`) and
      recorded in the license manifest before adding.
- [ ] tests added (integration on the S3 mock); coverage not regressed
- [ ] docs / ADR updated if behaviour or architecture changed
- [ ] `./format_code.sh` green

## Notes / log
Split out of T-0009 (manifest + version resolution) so the manifest structure /
statistics / resolution could land now (design-ratified, GATE Cat. 1/2/3) without
blocking on the S3 client dependency + adapter. T-0009 proved the manifest logic
through the trait on `MemoryStore`; this closes the "integration on the real mock"
half of its acceptance criterion 3/5. Coordinate with T-0010 (atomic commit),
which needs the same adapter + the CAS-fidelity test.
