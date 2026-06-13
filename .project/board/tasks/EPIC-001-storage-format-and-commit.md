---
id: EPIC-001
title: Object-storage-native storage format & ACID commit protocol
type: epic
status: backlog
priority: P0
assignee:
epic:
deps: []
rubric_refs: [1, 2]
estimate: L
created: T0
updated: T0
---

## Context

All durable state in caerostris-db must live on S3-compatible object storage — no POSIX filesystem durability. This epic covers the complete design and implementation of the custom on-object layout (Cat. 2) together with the ACID commit protocol (Cat. 1) that makes reads and writes safe under single-writer / multi-reader concurrency.

The storage format must be designed for **few, large, parallelizable range GETs**: columnar or adjacency-list structures partitioned so the query planner can issue a small number of large reads rather than many small random ones. Per R4, commit is an **atomic manifest swap** — an object-store-native technique where the writer finalizes a new root/manifest pointer in a single atomic PUT/conditional-PUT, leaving old versions pinned for in-flight readers until GC. The layout must directly serve the latency-envelope budget derived in EPIC-003 and SPIKE-0001 (B_max ≤ 75 MB at 1 Gbps, ≤ 4 MB at 50 Mbps), and must support the TLA+-modelled commit protocol from EPIC-004 / SPIKE-0002.

Downstream epics — secondary indices (EPIC-005), aggregates (EPIC-002/Cat.6), caching (EPIC-008) — all build on the abstractions defined here. The storage abstraction (object-store trait) must isolate the engine from concrete S3 clients so that a local MinIO/moto mock can stand in during CI (R12, Cat. 10). ADR and full format spec are mandatory deliverables (Cat. 2 requires both to score 100).

Relevant requirements: R2 (ACID), R4 (custom format), R11 (formal verification of commit), R12 (integration tests on mock).

## Acceptance criteria

- [ ] Format spec and ADR committed to `docs/` describing the on-object layout (columnar/adjacency partitioning, object naming, manifest/root structure, versioning, GC policy).
- [ ] Writer produces valid objects; reader round-trips an arbitrary graph with full fidelity (nodes, directed typed edges, properties on both).
- [ ] Commit = atomic manifest swap; old manifest versions remain readable by snapshot-pinning readers until explicitly GC-ed.
- [ ] Concurrent-reader safety: a reader that has pinned version V always sees a consistent snapshot even while a writer commits V+1.
- [ ] Range-read access patterns implemented: the planner can request a byte range of a data object; format partitioning keeps a relevant range-GET ≤ B_max for in-envelope queries.
- [ ] Layout demonstrably serves the latency envelope (per the cost model from SPIKE-0001): a benchmark or analysis shows bytes-read for a representative query fits within B_max.
- [ ] Crash/partial-write recovery tested: a simulated mid-commit failure leaves the database in the pre-commit state (no partial data visible).
- [ ] `./format_code.sh` green; CI passes with integration tests against the local S3 mock.

## Notes / log

Design-before-code: SPIKE-0001 (latency/cost model) and SPIKE-0002 (commit protocol TLA+) must be steering-ratified before implementation tasks in this epic move to `in_progress`. SPIKE-0003 (storage format spec) carries this dependency explicitly.
