---
id: SPIKE-0005
title: Commit-protocol pre-ratification constraints — CAS primitive, fencing token, durability barrier
type: spike
status: in_review
priority: P0
assignee: researcher
epic: EPIC-004
deps: []
rubric_refs: [1, 7, 11]
estimate: S
created: 2026-06-13T18:30:19Z
updated: 2026-06-13T19:15:00Z
---

## Context

Filed by `steering-distributed-acid` during the ratification pass of
`commanders-intent.md` + `master-rubric.md`. These are the **specific
falsification scenarios I will run against the SPIKE-0002 commit-protocol ADR +
TLA+ model**, surfaced now so the design author addresses them up front. They are
mandatory pre-conditions for my ratification of the commit-protocol ADR — they do
NOT block the launch, but SPIKE-0002 cannot reach `done` (and EPIC-001/EPIC-004
commit-path implementation cannot become `ready`) until each is resolved in the
ADR/model.

This item is a constraints rider on `SPIKE-0002`, not a competing design. It
records the attack vectors and the acceptance bar so the loop converges fast.

### Constraint 1 — name the S3 atomic primitive precisely; verify mock fidelity (Cat. 1/2/7)

The whole protocol rests on "a single conditional PUT (if-none-match or
equivalent)" and "a conditional-swap token" (SPIKE-0002). This is under-specified
and physically subtle:

- `If-None-Match: *` gives **create-if-absent** only — it cannot
  compare-and-swap an *existing* monotonic manifest pointer (V → V+1).
- S3 `If-Match` (true CAS on an existing object) is comparatively recent; MinIO /
  moto conditional-write support is partial and version-dependent.

The ADR MUST: (a) name the exact primitive and request shape for both the manifest
swap and the lease op; (b) if the design uses uniquely-named immutable manifests +
a "latest" resolution (list/max or a CAS pointer object), specify that resolution
and its consistency assumptions; (c) include a **mock-fidelity check** — a small
integration test proving the local S3 mock used in CI actually enforces the chosen
conditional semantics (two concurrent conditional PUTs: exactly one wins). If the
mock cannot enforce it, the TLA+ model would prove a protocol the implementation
cannot realize — a silent Cat. 11 divergence. Resolve before ratification.

### Constraint 2 — fencing token must be carried into the commit predicate; leases alone are not safety (Cat. 1/7)

"No split-brain" cannot be met by a lease + expiry alone. Zombie-writer scenario:
W1 acquires the lease, starts a commit, then stalls (GC pause / VM freeze / clock
skew). The lease expires; W2 acquires it and commits V+1. W1 wakes still believing
it holds the lease and performs its manifest swap. If the swap predicate is "lease
still says me," W1 commits stale data over V+1 → corruption / split-brain.

The ADR MUST make the **manifest swap conditional on the current manifest
version/etag (CAS)**, not on the writer's lease belief, so a stale writer's swap is
rejected deterministically regardless of what it thinks about the lease. The lease
is a *liveness* aid (who should be writing); the CAS-on-manifest is the *safety*
mechanism (at most one commit per version). The TLA+ invariant in SPIKE-0002
(`writer_count` never exceeds 1) is the wrong shape: two writers may transiently
*believe* they hold the lease — that is acceptable — what must hold is **at most
one writer's commit succeeds per manifest version** (no two distinct successful
commits share a predecessor manifest). State the safety invariant that way.

### Constraint 3 — durability ordering barrier (Cat. 1)

R2 requires "durable on ack"; the rubric requires readers never see a torn commit.
The ADR MUST state the ordering invariant explicitly: **every data object
referenced by manifest V+1 is fully PUT and durably readable before the manifest
swap is issued**, and the *commit ack to the client is the manifest-swap ack*.
Otherwise a reader can resolve V+1 and 404 on a not-yet-visible data object. Make
this a model invariant (a reader resolving manifest M can read every object M
references) and a recovery obligation (orphaned data objects from a crashed
pre-swap commit are GC-able and never referenced).

## Acceptance criteria
- [ ] SPIKE-0002 ADR names the exact S3 primitive(s) for manifest swap + lease,
      with request shapes, and documents the "latest manifest" resolution + its
      consistency assumptions (Constraint 1).
- [ ] A mock-fidelity integration test is specified (and, when the env exists,
      implemented) proving the CI S3 mock enforces the chosen conditional
      semantics: two concurrent conditional PUTs → exactly one succeeds (Constraint 1).
- [ ] The ADR + TLA+ model make the manifest swap conditional on the current
      manifest version/etag, and the safety invariant is restated as "at most one
      commit succeeds per manifest version" (no two distinct successful commits
      share a predecessor) — not merely `writer_count <= 1` (Constraint 2).
- [ ] A zombie/fenced-writer scenario (lease expired, stale writer attempts swap)
      is modelled and shown to be rejected by the CAS predicate (Constraint 2).
- [ ] The durability ordering barrier is an explicit ADR invariant + a TLA+
      invariant: a reader resolving manifest M can read every object M references;
      orphaned pre-swap objects are never referenced and are GC-able (Constraint 3).
- [ ] No Rust implementation required here — resolution lands inside SPIKE-0002's
      ADR + model.

## Notes / log
- T0+ratification: filed by steering-distributed-acid. These are the falsification
  scenarios SPIKE-0002 must survive for me to ratify. Non-blocking for launch;
  blocking for commit-path implementation readiness. Coordinate with
  steering-formal-methods (TLA+ invariant shape) and steering-storage (manifest
  swap is the storage atomic unit).
- Non-blocking notes recorded in
  `.project/decisions/0004-distributed-acid-ratification-findings.md`
  (master-less GC interaction; reject-not-queue for the writer lease).
- 2026-06-13T19:15:00Z (researcher): Research complete. Spec committed at
  `docs/specs/SPIKE-0005-commit-protocol-pre-ratification-constraints.md`.
  Steering sign-off request filed at
  `.project/decisions/0012-spike-0005-steering-sign-off-request.md`.
  Status set to `in_review`. Awaiting ratification from `steering-distributed-acid`
  (primary) and `steering-formal-methods` (secondary) before SPIKE-0002 ADR
  revisions can be merged and commit-path implementation becomes `ready`.
  Summary of recommendations:
    - Constraint 1: Use `If-None-Match: *` with uniquely named immutable
      manifest objects and lexicographic-max list resolution. Specify and run
      the mock-fidelity integration test before any commit-path task is `ready`.
    - Constraint 2: Embed generation counter in manifest key name; swap
      predicate is key uniqueness via `If-None-Match: *`. Safety invariant
      restated as ManifestVersionUniqueness. TLA+ must include ZombieWriter
      process.
    - Constraint 3: All data object PUTs acked before manifest swap issued;
      client ack = swap ack. TLA+ must include DataObjectDurable predicate,
      reader-safety invariant, and recovery invariant for orphaned objects.
