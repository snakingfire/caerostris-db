---
id: SPIKE-0002
title: Design S3 commit protocol and TLA+ model for atomicity + isolation
type: spike
status: ready
priority: P0
assignee:
epic: EPIC-004
deps: []
rubric_refs: [1, 11]
estimate: M
created: T0
updated: T0
---

## Context

This spike is the **design-before-code gate** for all commit/transaction implementation in EPIC-001 and EPIC-004. No implementation task that touches the commit path may move to `in_progress` until this spike is steering-ratified.

The S3 commit protocol must provide full ACID semantics under single-writer / multi-reader concurrency without relying on distributed locks or coordination services beyond what an S3-compatible object store natively provides (conditional PUTs, strong read-after-write consistency). The design must cover:

**Protocol design:**
- **Atomic manifest swap**: the writer finalises a new root/manifest pointer via a single conditional PUT (if-none-match or equivalent); either all of the new version is visible or none is. Old manifests remain readable for snapshot-pinned readers.
- **Writer leasing / fencing**: a mechanism on the object store (a lease object, a heartbeat, a conditional-swap token) that guarantees at most one writer-master at a time. The protocol must handle lease expiry, crash of the current writer, and a new writer taking over without leaving the database in an inconsistent state.
- **Reader snapshot pinning**: how a reader records which manifest version it is reading, so GC does not delete objects that are still referenced. The protocol must allow GC to proceed without blocking readers.
- **Failure modes**: what happens at each step if the writer crashes (mid-write, mid-swap, post-swap-pre-ack). The database must never be left in a state visible to readers that is partially committed.

**TLA+ model:**
- Specify the commit protocol as a TLA+ module covering: the state machine for a single writer commit, a concurrent reader taking a snapshot, writer-lease acquisition/expiry, and GC running concurrently.
- Define invariants: (a) **Atomicity** — no reader ever sees a partial write; (b) **Snapshot isolation** — a reader that has pinned version V sees only objects from V even while V+1 is being committed; (c) **Fencing** — the `writer_count` invariant never exceeds 1 simultaneously.
- The model must be small enough to model-check with Apalache in bounded mode (suggest: ≤2 concurrent readers, ≤2 writer epochs, ≤2 GC cycles).

Steering sign-off: **steering-distributed-acid** and **steering-formal-methods** must both approve the protocol ADR and the TLA+ model spec before implementation tasks become `ready`.

## Acceptance criteria

- [ ] Protocol ADR committed to `docs/adrs/` covering: manifest swap mechanism, lease/fencing strategy (with the specific object-store primitives used), reader snapshot pinning, GC safety, and all identified failure modes with their resolutions.
- [ ] TLA+ module committed to `docs/formal/` (`.tla` file): state machine for writer commit, concurrent reader, lease FSM, and GC; invariants for atomicity, snapshot isolation, and fencing defined as TLA+ INVARIANT clauses.
- [ ] Apalache model-check run documented: command line, model size (number of states explored), and result (no invariant violations) recorded in the ADR or a companion `.md` file. Even if Apalache is not yet in the Nix shell, the TLA+ spec must be syntactically valid and the check planned.
- [ ] Document committed and cross-referenced from EPIC-004 and EPIC-001.
- [ ] Steering-ratification record committed: both steering-distributed-acid and steering-formal-methods sign-off recorded in `.project/decisions/`.
- [ ] No implementation Rust code required — design + model only.

## Notes / log

Output feeds EPIC-004 (Rust implementation of the commit protocol) and EPIC-006 (writer leasing for attach modes). The TLA+ model must be kept in sync with the implementation throughout the project; drift is a bug.
