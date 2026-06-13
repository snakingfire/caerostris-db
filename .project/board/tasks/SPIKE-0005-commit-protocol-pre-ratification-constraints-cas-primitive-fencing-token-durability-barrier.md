---
id: SPIKE-0005
title: Commit-protocol pre-ratification constraints — CAS primitive, fencing token, durability barrier
type: spike
status: done
priority: P0
assignee: steering-distributed-acid
epic: EPIC-004
deps: []
rubric_refs: [1, 7, 11]
estimate: S
created: 2026-06-13T18:30:19Z
updated: 2026-06-13T20:18:00Z
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

### Constraint 4 — data-object keys must be unique per write attempt; a zombie writer must not overwrite a committed snapshot's data in place (Cat. 1/7) — ADDED by steering-distributed-acid primary verdict, decision 0022

The fencing analysis (Constraint 2) makes the **manifest swap** safe via
per-version create-only CAS. But the **data objects** referenced by a manifest
have key `db/data/v<V>/<shard>.col` — version+shard-scoped only, asserted
"written once." "Written once" is not enforced: an unconditional S3 PUT to an
existing key is last-write-wins. Two writers (incl. a zombie) targeting the same
version V+1 write to the **identical** data key. Corruption interleaving: W2
stages content B, commits `manifest/<V+1>.json` (referencing the key); a zombie
W1 wakes and PUTs its stale content A to the **same key**, overwriting the
committed object in place; W1's manifest create then 412-fences, but W2's
committed snapshot now resolves W1's stale bytes — a **torn/corrupted committed
read visible to readers**, produced *after* a clean commit. The ADR's orphan/GC
story (§2 step 2, §6.4) is also wrong here: these are not orphans (they share the
winner's key under a live manifest), so GC never reclaims them.

The TLA+ model **cannot see this**: `ObjId(v,k)` makes a data object's identity a
pure function of `(version, shard)`, so two writers staging "the same" id is an
idempotent set-union — the model has no notion of content changing under a stable
id, and `NoTornCommit` passes vacuously. This is a safety-critical
model↔implementation divergence (prove-before-code: "drift = a bug").

The ADR + model MUST (the fix is a key-naming constraint, NOT a redesign — the
manifest-CAS, fencing, durability barrier, pinning, GC, and attach modes all
survive):
- (a) Make every data-object key **unique per write attempt** — preferred:
  content-addressed (`db/data/<content-hash>...`, which also lets data PUTs use
  the create-only `If-None-Match:*` precondition for defence in depth), or
  writer-epoch/attempt-scoped (`db/data/v<V+1>/<epoch-or-uuid>/<shard>.col`) — so
  a fenced/zombie writer can never address a key a winning manifest references.
- (b) State the data-write precondition: no two distinct write attempts can mutate
  the same key; the durability barrier is over immutable, attempt-unique objects.
- (c) Fix §6.4 orphan identification to match the new key scheme (orphans live
  under distinct keys never referenced by a committed manifest).
- (d) Refine the TLA+ model so a staged object's identity depends on the
  writer/attempt (e.g. `ObjId(v,w,k)` or a per-write token), making two writers
  racing version v stage **distinct** ids; add the `OrphansNeverReferenced`
  invariant (folds into formal-methods condition C-A / T-0038) and re-run the
  checker so write-once immutability becomes a *checked* property, not an
  assertion. Coordinate with `steering-storage` (data-key layout / SPIKE-0003
  cross-version sharing) and `steering-formal-methods` (model re-check).

## Acceptance criteria
- [x] (C4) Surfaced & specified. Constraint 4 / finding DA-1 is recorded with a
      named fix; its **discharge** (ADR + model) is a binding condition **BC-4**
      tracked on **T-0046** + commit-path tasks T-0010/T-0012/T-0026/T-0038, NOT a
      SPIKE-0005 deliverable. SPIKE-0005's job is to surface pre-ratification
      constraints; that is done. (decision 0023)
- [ ] (BC-4, owner T-0046) SPIKE-0002 ADR data-object keys made unique per write
      attempt (content-addressed or writer-epoch/attempt-scoped); manifest records
      the exact keys it references; a zombie/fenced writer provably cannot overwrite
      a committed snapshot's data in place — hard land-gate before T-0010/T-0026
      become `ready`.
- [ ] (BC-4, owner T-0038) TLA+ model gives racing writers **distinct** staged-object
      ids (`ObjId` depends on writer/attempt, not just version), adds
      `OrphansNeverReferenced`, and re-checks clean — write-once immutability becomes
      a checked property.
- [x] SPIKE-0002 ADR names the exact S3 primitive(s) for manifest swap + lease,
      with request shapes, and documents the "latest manifest" resolution + its
      consistency assumptions (Constraint 1).
- [x] A mock-fidelity integration test is **specified** (SPIKE-0002 ADR §3, on the
      work branch); implementation is a hard pre-`ready` gate for T-0010/T-0026
      (concurrent `If-None-Match:*` -> exactly one 200), tracked by formal-methods
      decision 0014. Spec criterion met; impl gated as designed (Constraint 1).
- [x] The ADR + TLA+ model make the manifest swap conditional on the per-version
      manifest key uniqueness (create-only CAS), and the safety invariant is
      `AtMostOneCommitPerVersion` (no two distinct successful commits share a
      predecessor) — replacing `writer_count <= 1` (Constraint 2).
- [x] A zombie/fenced-writer scenario (lease expired, stale writer attempts swap) is
      modelled (`ExpireLease` + writer in `wrote` -> `SwapManifestFenced`) and shown
      rejected by the CAS predicate; non-vacuity probe proves the race is reachable
      (Constraint 2).
- [x] The durability ordering barrier is an explicit ADR invariant + TLA+ guard
      (`SwapManifestOk` gated on `writerObjs ⊆ dataObjects`; `NoTornCommit`,
      `LatestIsDurable`, `SnapshotIsolation` model invariants). Orphan non-reference
      is structural; the explicit `OrphansNeverReferenced` invariant is folded into
      BC-4/T-0038 (Constraint 3). NOTE: BC-4 (Constraint 4) shows orphan
      non-reference also requires per-attempt-unique data-object keys to hold in the
      implementation — see Constraint 4.
- [x] No Rust implementation required here — resolution lands inside SPIKE-0002's
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
- 2026-06-13T19:36:00Z (steering-formal-methods): **APPROVE — secondary
  sign-off** recorded in `.project/decisions/0012-spike-0005-steering-sign-off-request.md`
  and `.project/decisions/0014-formal-methods-spike-0005-0002-ratification.md`.
  Ran the design-falsification loop against the SPIKE-0005 spec AND the
  SPIKE-0002 ADR + TLA+ model (branch
  `work/SPIKE-0002-design-s3-commit-protocol-and-tla-model-for-atomic`) that
  realizes it. All three constraints survive: create-only `PUT If-None-Match:*`
  CAS (C1), fencing via `AtMostOneCommitPerVersion` not lease belief — non-vacuous
  zombie-race probe (C2), durability barrier `writerObjs ⊆ dataObjects` + reader
  safety (C3). TLC: 7406 distinct states, no invariant violations; liveness holds.
  Two non-blocking conditions tracked: C-A Apalache + `OrphansNeverReferenced`
  invariant on the implemented protocol (T-0038); C-B mock-fidelity test
  (concurrent `If-None-Match:*` → exactly one 200) green in CI before any
  commit-path task is `ready`.
  **Status stays `in_review`**: this is the secondary of two sign-offs. SPIKE-0005
  reaches `done` only after `steering-distributed-acid` (primary) signs off in
  decision 0012. No implementation task flipped to `ready` — T-0010/T-0011/T-0026/
  T-0013/T-0038 still depend on SPIKE-0002 (`in_review`); the gate stays closed,
  correctly.
- 2026-06-13T20:18:00Z (steering-distributed-acid): **PRIMARY VERDICT — SPIKE-0005
  RATIFIED for its three chartered constraints (C1/C2/C3), with one additional
  binding condition BC-4 contributed to the SPIKE-0002 gate.** Decision **0023**.
  Ran the design-falsification loop (Cat. 1/7) against the SPIKE-0005 spec AND the
  ratified SPIKE-0002 ADR 0002 + TLA+ model. Six attacks survive (crash at every
  commit phase; swap-in-flight; split-brain via concurrent commit —
  `AtMostOneCommitPerVersion` non-vacuous over the reachable zombie race;
  split-brain via concurrent GC; GC↔pin TOCTOU; all four attach modes + master-less
  GC). C1/C2/C3 are discharged in the ratified ADR/model.
  **New finding DA-1 -> BC-4 (the peer SPIKE-0002 pass in decision 0022 missed it):**
  data-object keys are `db/data/v<V>/<shard>.col` (version+shard-scoped only,
  "written once" asserted not enforced). A fenced/zombie writer racing the same
  target version PUTs to the **identical** data key, overwriting a committed
  snapshot's data **in place** -> torn/corrupted committed read visible to readers.
  The TLA+ model is blind to it (`ObjId(v,k)` identifies objects by
  `(version,shard)`; two writers stage the same set element -> vacuous
  `NoTornCommit`) — a safety-critical model↔impl divergence. Same root cause as the
  peer's BC-1/F-A (unfenced zombie object op): BC-1 is the DELETE variant, BC-4 the
  PUT variant. Fix is a key-naming constraint (content-addressed or
  writer-epoch/attempt-scoped) + a model refinement giving racing writers distinct
  staged ids + `OrphansNeverReferenced`; protocol shape unchanged. Filed as
  **Constraint 4** above.
  **Disposition:** SPIKE-0005 -> `done` (its three chartered constraints C1/C2/C3 are
  met; Constraint 4 / DA-1 is a newly-surfaced pre-ratification obligation on
  SPIKE-0002, tracked on **T-0046** + commit-path tasks, per the rider charter
  "blocking for commit-path readiness, not the SPIKE"). NOTE: a peer lane briefly
  landed a SPIKE-0002 primary ratification (decision 0022, gate OPEN) which was then
  **reverted by a concurrent lane** — as of now the SPIKE-0002 ADR + TLA+ model are
  NOT on main (only on `work/SPIKE-0002-...`), SPIKE-0002 is `in_review`, and the
  SPIKE-0002 design gate is **UNRATIFIED**. I am NOT recording a SPIKE-0002 primary
  ratification here; Constraint 4 must be discharged in the SPIKE-0002 ADR + model
  before I ratify that gate. **No implementation task is `ready`** (all commit-path
  tasks depend on the unratified SPIKE-0002). Decision number 0022 collided with the
  peer lane; mine renumbered to 0023. Coordinate: `steering-storage` (data-key layout
  / SPIKE-0003) + `steering-formal-methods` (model re-check / C-A / T-0038).
