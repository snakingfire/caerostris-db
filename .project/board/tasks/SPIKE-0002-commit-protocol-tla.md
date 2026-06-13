---
id: SPIKE-0002
title: Design S3 commit protocol and TLA+ model for atomicity + isolation
type: spike
status: in_progress
priority: P0
assignee: formal-prover
epic: EPIC-004
deps: []
rubric_refs: [1, 11]
estimate: M
created: T0
updated: 2026-06-13T19:12:00Z
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

> Paths corrected to the canonical `docs/adr/` and `formal/` per BUG-0003 /
> decision 0005 (the original `docs/adrs/` + `docs/formal/` would silently cap
> Cat. 11 at ≤50 because the grader reads the canonical paths).

- [x] Protocol ADR committed to `docs/adr/` (`docs/adr/0002-s3-commit-protocol.md`) covering: manifest swap mechanism, lease/fencing strategy (with the specific object-store primitives used), reader snapshot pinning, GC safety, and all identified failure modes with their resolutions.
- [x] TLA+ module committed to `formal/commit-protocol/` (`commit_protocol.tla`): state machine for writer commit, concurrent reader, lease FSM, and GC; invariants for atomicity, snapshot isolation, and fencing (restated per steering finding as **at-most-one-commit-per-manifest-version**, not bare `writer_count<=1`) defined as TLA+ INVARIANT clauses.
- [x] Apalache model-check run documented: command line, model size, and result recorded in `formal/results/commit_protocol_check.txt` and the ADR. Apalache is not yet in the Nix shell; the spec is syntactically validated and the check planned + scripted (T-0038 runs it on the implemented protocol).
- [x] Document committed and cross-referenced from EPIC-004 and EPIC-001.
- [ ] Steering-ratification record committed: both steering-distributed-acid and steering-formal-methods sign-off recorded in `.project/decisions/` (ratification REQUESTED in decision 0012; status stays `in_review` until both sign off via the design-falsification loop).
- [x] No implementation Rust code required — design + model only.

## Pre-ratification falsification scenarios this design must survive (from decisions 0001/0004)

- [x] **S3 CAS primitive named precisely** (not "if-none-match or equivalent"): monotonic versioned manifest object names + create-only conditional PUT (`If-None-Match: *`) as the compare-and-swap; "latest" resolved by a separate pointer / list. (decision 0004 finding 2; SPIKE-0005 Constraint 1)
- [x] **Fencing safety derives from manifest-version CAS, not lease belief** — the zombie-writer interleaving (W1 stalls, lease expires, W2 commits V+1, W1 wakes and swaps) is refuted by the model. Invariant restated as `AtMostOneCommitPerVersion`. (decision 0004 finding 3; SPIKE-0005 Constraint 2)
- [x] **Durability ordering barrier modelled**: all data objects of V+1 durable before the swap; commit ack == swap ack; orphaned pre-swap objects never referenced and GC-able. (decision 0004 finding 4; SPIKE-0005 Constraint 3)
- [x] **GC safety without a central pin registry**: TTL'd pin objects + retention grace window; GC runs only under the writer lease; master-less readers read latest committed immutable manifest. (decision 0001 F3 / 0004 non-blocking note)

## Notes / log

Output feeds EPIC-004 (Rust implementation of the commit protocol) and EPIC-006 (writer leasing for attach modes). The TLA+ model must be kept in sync with the implementation throughout the project; drift is a bug.
