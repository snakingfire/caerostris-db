# Decision 0004 — Distributed-ACID ratification-pass findings

- **Date:** 2026-06-13 (T0+ratification pass)
- **Owner:** steering-distributed-acid
- **Type:** steering ratification (Cat. 1 ACID/correctness, Cat. 7 concurrency &
  attach modes) of `docs/commanders-intent.md` + `docs/requirements/master-rubric.md`
- **Verdict:** APPROVE (surface-and-track; we do not hard-block the launch)
- **Related:** `BUG-0003`, `SPIKE-0005`, `SPIKE-0002`, `EPIC-001`, `EPIC-004`,
  `EPIC-006`

## What I reviewed

The two source documents, against my mandate, applying the design-falsification
loop to the parts of intent/rubric that bind Cat. 1 and Cat. 7 and to the
isolation/snapshot-pinning surface of the latency theorem. I also read the
seeded board items that operationalize my domain (SPIKE-0002, EPIC-001,
EPIC-004, EPIC-006).

## Conclusion on the source docs

The mission, the GATE structure for Cat. 1 (weight 14) and Cat. 7 (weight 8),
and the latency theorem framing are **internally consistent and feasible** in my
domain. The single-writer/multi-reader + atomic-manifest-swap shape is the right,
tractable architecture for S3-backed ACID, and the rubric anchors for Cat. 1/7
correctly demand property tests, crash/partial-write recovery, split-brain
prevention, and TLA+ alignment. Nothing in intent or rubric is infeasible or
self-contradictory in my domain at the GATE level. Hence APPROVE.

However, the *operationalizing* docs (board SPIKEs) contain one process defect
and three under-specifications that, if uncaught, would let a structurally
unsound commit protocol slip past my future ratification or silently underscore
my GATE categories. I surfaced them as trackable items rather than blocking the
launch.

## Blocking-for-ratification findings (tracked, not launch-blocking)

1. **Path mismatch → silent GATE underscore (BUG-0003, P0).** SPIKE-0001/0002/0003
   and EPIC-004 point ADR/TLA+ artifacts at `docs/adrs/` and `docs/formal/`; the
   canonical, grader-enforced paths are `docs/adr/` and `formal/`
   (formal-verification-policy, ADR README). If the model lands at `docs/formal/`,
   the grader caps Cat. 11 at ≤50 and my Cat. 1 "matches the TLA+ model" sign-off
   loses its referent. Pure doc/board-text fix; independent of design.

2. **S3 CAS primitive under-specified + mock fidelity unverified (SPIKE-0005,
   Constraint 1, P0).** "conditional PUT (if-none-match or equivalent)" is not a
   primitive. `If-None-Match:*` is create-if-absent, not CAS on an existing
   manifest pointer; `If-Match`/MinIO/moto support is partial and version-bound.
   The ADR must name the exact primitive + "latest manifest" resolution and prove
   the CI mock enforces it (two concurrent conditional PUTs → exactly one wins),
   else the TLA+ model proves a protocol the impl can't realize.

3. **Fencing-by-lease-alone is a split-brain falsification (SPIKE-0005,
   Constraint 2, P0).** Zombie-writer: W1 stalls, lease expires, W2 commits V+1,
   W1 wakes and swaps. Safety must come from conditioning the manifest swap on the
   current manifest version/etag (CAS), not on lease belief. The SPIKE-0002
   invariant `writer_count <= 1` is the wrong shape — restate as "at most one
   commit succeeds per manifest version."

4. **Durability ordering barrier unstated (SPIKE-0005, Constraint 3, P1).** All
   data objects of V+1 must be durably readable before the manifest swap; commit
   ack == manifest-swap ack. Otherwise a reader resolves V+1 then 404s a data
   object. Make it a model invariant + a recovery obligation (orphaned pre-swap
   objects never referenced, GC-able).

These four are the falsification scenarios SPIKE-0002 must survive before I
ratify the commit-protocol ADR. None blocks the launch; all block commit-path
implementation readiness (consistent with the prove-before-code gate).

## Non-blocking notes (for the SPIKE-0002 author)

- **Master-less mode × GC (R3 mode 3).** No live writer means no process to honor
  reader pins. Resolution: GC runs only under the writer lease; master-less
  readers read the latest committed immutable manifest + immutable objects; GC
  uses a grace window / pin objects readable by any process. Make it an explicit
  obligation in the ADR.
- **EPIC-006 "rejected (or queued)" for a second writer.** A server-side queue is
  a coordination service the object-store-native design avoids. Close to "reject;
  optional client-side retry with backoff," not server-side queue.
- **Snapshot isolation level.** Rubric floor is SI; the ADR should state the exact
  level achieved (single-writer + immutable versioned manifests naturally give
  serializable snapshots for reads — claim only what the model proves).

## Why APPROVE despite open findings

Per my mandate and the operating model: we surface and track, we do not hard-block
the launch. The source docs are sound in my domain; the open items are design
obligations on a not-yet-built protocol, correctly gated by prove-before-code.
The board now carries each as a tracked P0/P1 item, so the run proceeds with the
risks visible rather than hidden.
