---
id: EPIC-004
title: ACID transactions + TLA+ formal verification
type: epic
status: backlog
priority: P0
assignee:
epic:
deps: []
rubric_refs: [1, 11]
estimate: L
created: T0
updated: T0
---

## Context

caerostris-db must provide full ACID semantics (Cat. 1, weight 14, GATE) for committed transactions on S3-backed storage, and those semantics must be backed by a machine-checked formal proof (Cat. 11, weight 6, GATE). The single-writer / multi-reader concurrency model is by design — it is what makes the S3 commit protocol tractable — but it introduces correctness obligations that informal argument cannot satisfy.

This epic covers: (1) the formal **commit protocol design** — atomic manifest swap, writer leasing/fencing on the object store to prevent split-brain, reader snapshot pinning; (2) a **TLA+/Apalache model** that specifies atomicity and snapshot isolation and is model-checked with no invariant violations; (3) the **Rust implementation** of the protocol, matching the TLA+ model; and (4) **crash / partial-write recovery** tests that demonstrate the database is never left in a partially-committed state.

SPIKE-0002 is the design-before-code task that produces the protocol ADR and the initial TLA+ model; it must be steering-ratified (steering-distributed-acid + steering-formal-methods) before implementation tasks in this epic or EPIC-001 that depend on the commit semantics move to `in_progress`. The TLA+ model must be kept in sync with the code throughout development — drift is treated as a bug.

Relevant requirements: R2 (ACID, single-writer/multi-reader), R3 (attach modes — leasing must cover all four), R11 (formal verification), R4 (commit = manifest swap).

## Acceptance criteria

- [ ] Protocol ADR committed: atomic manifest swap mechanism, writer leasing/fencing strategy, reader snapshot pinning, GC safety, and failure modes documented.
- [ ] TLA+/Apalache model committed to `docs/formal/` covering atomicity (no partial commit visible to readers), snapshot isolation (readers see consistent version V while writer commits V+1), and fencing (no two writer-masters simultaneously).
- [ ] Model checked by Apalache with no invariant violations for the full protocol state space (within a bounded model check).
- [ ] Rust implementation passes all ACID property tests: unit tests for each invariant (atomicity, consistency, isolation, durability) plus property-based tests with arbitrary interleavings.
- [ ] Crash/partial-write recovery tested: simulated failures at each commit phase leave the database in the pre-commit state.
- [ ] Writer-leasing prevents split-brain: a test demonstrates that a second writer attempting to claim the lease while one is held is rejected.
- [ ] TLA+ model kept in sync: a CI check or manual gate verifies the implementation's commit phase sequence matches the model's spec.
- [ ] `./format_code.sh` green; CI passes.

## Notes / log

SPIKE-0002 is the first task. Its output (protocol ADR + TLA+ model) must be steering-ratified before implementation tasks here or in EPIC-001 that implement the manifest swap become `ready`.
