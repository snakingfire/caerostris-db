---
id: EPIC-006
title: Concurrency & the four attach modes (embedded ×3 + server)
type: epic
status: backlog
priority: P1
assignee:
epic:
deps: []
rubric_refs: [7]
estimate: L
created: T0
updated: T0
---

## Context

caerostris-db must support all four attach modes (Cat. 7, weight 8, GATE): embedded writer-master, embedded read-only, embedded on a master-less database, and server mode (server is the writer-master and also serves reads to remote clients). Concurrent readers must work correctly across all modes. This epic covers the coordination layer that enforces the single-writer constraint and the server-mode network layer.

The single-writer constraint is by design (R2) — it makes the S3 commit protocol tractable — but must be **enforced**: two processes must not simultaneously hold the writer role ("split-brain"). The mechanism from EPIC-004 (writer leasing/fencing on the object store) is the building block. Embedded read-only and master-less modes need only open the latest committed manifest; they never need the writer to be alive (R3). Server mode requires a network listener that exposes the query interface to remote embedded-read-only clients.

Concurrent readers must be verified under load: multiple readers issuing queries in parallel against a writer-master must each see consistent snapshots and not interfere with each other or with the writer.

Relevant requirements: R3 (all four attach modes), R2 (single-writer/multi-reader), R1 (embedded + server from one codebase).

## Acceptance criteria

- [ ] Embedded writer-master mode: process opens DB, acquires writer lease, can read and write; tested end-to-end.
- [ ] Embedded read-only mode: process attaches to a DB whose writer-master is a separate process; reads see consistent snapshots; tested with a concurrent writer in a separate thread/process.
- [ ] Embedded master-less mode: process opens a DB with no live writer; reads succeed against the latest committed manifest; tested.
- [ ] Server mode: server process holds the writer role and serves read queries to remote clients over a network protocol; at least one remote read-only client can query successfully.
- [ ] Split-brain prevention: a test demonstrates that a second process attempting to acquire the writer lease while one is held is rejected (or queued, per the leasing design).
- [ ] Concurrent readers: a load test with N parallel readers (N ≥ 4) against a writer-master shows no corrupted or inconsistent results; all readers see valid snapshots.
- [ ] Attach-mode API is clean: the Rust public API clearly expresses the mode chosen at open time; misuse (e.g. attempting a write in read-only mode) is a compile-time or early-runtime error.
- [ ] `./format_code.sh` green; CI passes all mode tests.

## Notes / log

Depends on EPIC-004 (writer leasing design from SPIKE-0002) being ratified before the leasing implementation here becomes `ready`. Server-mode network protocol choice (gRPC, custom TCP, HTTP) should be decided by ADR.
