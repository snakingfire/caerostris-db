# Decision 0001 — Storage-domain ratification of commander's intent + master rubric

- **Date / mark:** 2026-06-13T18:29Z (T+0:06)
- **Author:** `steering-storage`
- **Type:** Ratification pass (day-one mandate, `docs/process/steering-committee.md` §Day-one mandate item 1 & 2)
- **Scope:** Storage domain only — rubric Cat. 2 (storage format & S3 commit
  protocol), and the storage-bearing aspects of Cat. 1 (commit atomicity) and
  Cat. 3 (the layout must serve the latency byte budget). Query execution, ACID
  isolation semantics, and writer leasing are out of scope (owned by
  `steering-query-cypher` and `steering-distributed-acid`).
- **Verdict:** **APPROVE** intent + rubric for the storage domain. Run proceeds.
- **Status:** accepted

## Context

Day-one ratification. `steering-storage` adversarially reviewed
`docs/commanders-intent.md` and `docs/requirements/master-rubric.md` for anything
in the storage domain that is **infeasible, contradictory, or under-specified** —
with focus on the latency selectivity-envelope theorem (the part the on-object
layout must satisfy) and the Cat. 2 GATE. Per the operating doctrine we do not
hard-block the launch; we surface and track.

## What survived falsification (evidence the design is sound enough to build on)

- **Byte-budget arithmetic is correct.** 1 Gbps = 125 MB/s; reserving ~0.4 s for
  K round trips + compute leaves ~0.6 s of transfer ⇒ ~75 MB. 50 Mbps = 6.25 MB/s
  × ~0.6 s ⇒ ~3.75 MB ≈ 4 MB. Both stated figures check out. The "physics forbid
  an unconstrained 6-hop expansion" claim is sound — no storage layout makes 10^6+
  paths fit in 4–75 MB; the conditional (selectivity-anchored) framing is the only
  feasible one, and the rubric correctly makes the SLA conditional on the envelope.
- **Atomic-manifest-swap commit is a feasible object-store-native technique.**
  A single conditional PUT as compare-and-swap of a root pointer is implementable
  on modern S3 and on MinIO; old versions remain readable; this is the right shape
  for Cat. 1/2. (Subject to F2 below.)
- **Versioned, GC-able, self-describing format with reader version-pinning** is a
  coherent and achievable Cat. 2 design (subject to F3 below).
- **No storage-domain contradiction** between intent and rubric was found that
  would make the project structurally impossible. Approval is therefore correct.

## Findings (tracked in SPIKE-0008; none blocks the launch)

These are storage-domain **under-specifications** the intent/rubric leave implicit
(tracked on the board as **SPIKE-0008**). Each maps to a Cat. 2 (and Cat. 1/3)
"100" anchor that cannot be honestly scored 100 until discharged in the relevant
ratified SPIKE. They are *blocking for ADR ratification later*, not blocking for
the run starting now.

**F1 — Early-abort partial adjacency reads are mandatory for the binding 50 Mbps
case (under-specified).** The intent's own degree-10 / 6-hop figure gives
fan-out^6 = 1e6. The naive product bound `|seed| ≤ B_max / (node_bytes ×
fan_out^6)` at B_max ≈ 4 MB and even ~16 B/node yields **< 1** — a full BFS 6-hop
expansion of a single seed does not fit at 50 Mbps. The envelope is feasible only
if (a) LIMIT-driven early termination prunes the realized frontier far below the
worst-case product, and (b) the on-object layout lets a reader **abort an
adjacency-list range-GET early**. SPIKE-0003 must specify adjacency chunking/page
sizing; SPIKE-0001 must state the realized-fan-out assumption its proof leans on.
Owner: SPIKE-0003 (storage side) + SPIKE-0001 (cost-model side).

**F2 — The conditional-PUT primitive underpinning atomic commit must be pinned,
not assumed (under-specified feasibility dependency).** "If-none-match ⇒ atomic
CAS" is real on S3 (2024+ conditional writes) and MinIO, but not universal across
all S3-compatible stores / mock configs. SPIKE-0002/0003 must name the exact
primitive, confirm the CI mock supports it, and specify the fallback or hard
precondition. If the mock does not honor it, the Cat. 1/2 GATE atomicity claim is
unprovable on the mock — escalate to a joint storage + distributed-acid session.
Owner: SPIKE-0002 (commit protocol), co-owned with `steering-distributed-acid`.

**F3 — GC safety against slow/crashed readers with no central pin registry
(under-specified).** R3 master-less + embedded read-only modes mean no always-live
coordinator for GC to consult. A crashed reader leaves a stale pin; a slow reader's
pin may be invisible; GC could delete an object mid-read. SPIKE-0002/0003 must
specify a provably safe-GC policy (retention grace window / generational manifest
retention / TTL'd pin objects whose deletion deadline is strictly after the max
reader-session lifetime) and include a GC-vs-reader interleaving invariant in the
TLA+ model. Owner: SPIKE-0002 + SPIKE-0003, co-owned with `steering-distributed-acid`.

## Non-blocking guidance (not a gate)

**"Few, large, parallelizable range GETs" vs. a scattered selective seed set are in
apparent tension.** A highly selective filter yields a small *scattered* seed set,
whose adjacency lists sit at scattered offsets — naively, many small random GETs.
The resolution is known and is exactly SPIKE-0003's job: sort/cluster adjacency by
source ID, batch contiguous ranges, and issue parallel multi-range GETs within one
phase (the phase bound K then counts parallel batches, not individual GETs).
Recorded as design guidance for SPIKE-0003, not a separate finding.

## Alternatives considered

- **Hard-block the launch until F1–F3 are resolved.** Rejected: violates the
  no-hard-block doctrine and the "never block the board" non-negotiable; these are
  design-SPIKE constraints, not launch blockers, and the SPIKEs that must discharge
  them have not even started.
- **File F1–F3 as three separate BUG items.** Rejected: they are not defects in
  shipped artifacts; they are constraints on not-yet-written design specs. One P0
  tracking SPIKE (SPIKE-0008) cross-referenced from the relevant design SPIKEs keeps
  the board honest without fragmenting the tracking.

## Consequences

- Run proceeds; SPIKE-0001 and SPIKE-0002 remain `ready` and unblocked.
- SPIKE-0008 (P0) tracks F1–F3; SPIKE-0003 notes now reference them.
- `steering-storage` will refuse to ratify the storage-format ADR (the eventual
  SPIKE-0003 output) until F1, F2, and F3 are explicitly addressed or reasoned-away.
