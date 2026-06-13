# Decision 0023 — Distributed-ACID PRIMARY verdict on SPIKE-0005; contributes binding condition BC-4 to the SPIKE-0002 gate

- **Date / marker:** 2026-06-13 (≈ T0+1:50)
- **Owner / role:** `steering-distributed-acid` (PRIMARY signer, Cat. 1 ACID / Cat. 7
  concurrency & attach modes; dispatched on **SPIKE-0005**)
- **Type:** steering ratification (design-falsification Loop A) — primary sign-off on
  the SPIKE-0005 constraints rider, plus a reconciliation with the concurrently
  recorded SPIKE-0002 primary ratification.
- **Verdict on SPIKE-0005:** **RATIFIED** for its three chartered constraints
  (C1 CAS primitive, C2 fencing, C3 durability barrier) — discharged in the ratified
  ADR 0002 + TLA+ model. **Contributes one additional binding condition (BC-4 / finding
  DA-1)** that the SPIKE-0002 primary pass (decision 0022) did not surface.
- **Rubric:** Cat. 1 (ACID, GATE, w14), Cat. 7 (concurrency/attach, GATE, w8); Cat. 11
  (formal) via model fidelity.
- **Artifacts reviewed (on `main`):** `docs/specs/SPIKE-0005-...md`,
  `docs/adr/0002-s3-commit-protocol.md`, `formal/commit-protocol/commit_protocol.tla`
  (+ `.cfg`s), `formal/results/commit_protocol_check.txt`; peer decisions 0013, 0014,
  0020, 0021, **0022 (peer `steering-distributed-acid` SPIKE-0002 ratification)**.
- **Related:** SPIKE-0005, SPIKE-0002, decisions 0004, 0013, 0014, 0020, 0021, 0022;
  EPIC-001, EPIC-004, EPIC-006; T-0010/T-0011/T-0012/T-0013/T-0026/T-0038; new task
  T-0046 (BC-4 ADR+model fix).

## Concurrency note (two lanes, one role) — and the SPIKE-0002 land was reverted

During this review a parallel `steering-distributed-acid` lane briefly landed a
SPIKE-0002 PRIMARY ratification onto the working tree (decision 0022
RATIFIED-WITH-CONDITIONS; ADR 0002 -> `accepted`; SPIKE-0002 -> `done`; gate OPEN;
conditions BC-1 GC-delete fencing, BC-2 mock-fidelity test, BC-3 zombie integration
test). I read it in full and concurred that the commit protocol's *shape* is sound.
**That land was then reverted by a concurrent lane**: as of this decision, the
SPIKE-0002 ADR (`docs/adr/0002-s3-commit-protocol.md`) and TLA+ model
(`formal/commit-protocol/`) are **not on the working tree / main HEAD** (they remain
on the unmerged `work/SPIKE-0002-...` branch), SPIKE-0002 is back to `in_review`, and
ADR 0002 is no longer `accepted`. **So the SPIKE-0002 design gate is currently
UNRATIFIED.** I reviewed the ADR + model from the `work/SPIKE-0002-...` branch; that
review stands as a design review, but I am **not** recording a SPIKE-0002 primary
ratification here (that is a separate dispatched task, and the artifacts are not on
main). My scope is SPIKE-0005.

Net effect for DA-1: rather than a post-hoc binding condition on an already-open gate,
**DA-1 / BC-4 is a pre-ratification obligation on SPIKE-0002** — exactly the kind of
constraint the SPIKE-0005 rider exists to carry. It must be discharged in the
SPIKE-0002 ADR + model before I (as primary) ratify the SPIKE-0002 gate and before any
commit-path implementation task becomes `ready`. The decision-number collision with the
peer lane (0022) is resolved by renumbering mine to 0023.

## My independent falsification pass (Cat. 1/7) — what survived

Identical conclusions to decision 0022 §1-6 on the **commit/read/fencing core**:
- **A1 Atomicity (crash at every commit phase):** survives — manifest-create is the
  sole reachability point; staged-but-unreferenced objects invisible.
- **A2 Swap-in-flight:** survives — create-only PUT is atomic at the store (rests on
  BC-2 mock fidelity).
- **A3 Split-brain via concurrent commit (zombie writer):** survives —
  `AtMostOneCommitPerVersion` non-vacuous over the reachable zombie race; safety on
  store-enforced CAS, never lease belief.
- **A4 Split-brain via concurrent GC / A5 GC-vs-pin TOCTOU:** survives *in the model*;
  implementation obligations are BC-1 (delete fencing) and the pin-TTL/grace timing.
- **A6 Attach modes (all four) + master-less GC:** survives.
- **TLA+ alignment:** model exists, TLC-checked (7406 states, non-vacuous) for the
  commit path; the GC-delete and data-write *abstractions* over-approximate reality
  (see BC-1 and BC-4).

## The additional finding the peer pass missed — DA-1 (becomes BC-4)

Decision 0022 §1 concludes "the only thing that makes any data object reachable is its
manifest ... no torn-commit window exists." That is true for the **manifest**, but it
overlooks the **data object** layer:

**ADR §1 data-object key = `db/data/v<V>/<shard>.col` — version+shard-scoped only,
"written once" *asserted*, not *enforced*.** An unconditional S3 PUT to an existing key
is last-write-wins. Two writers targeting the same version V+1 write to the **identical**
key. Corruption interleaving:

1. W2 (epoch 2) stages `db/data/v<V+1>/shard.col` <- content B, all PUTs acked, creates
   `db/manifest/<V+1>.json` referencing it -> **commits V+1**; readers may pin and read B.
2. Zombie W1 (epoch 1, stalled at/just-before its own data-write for the same target
   V+1) wakes and PUTs its stale content A to the **same key** — an in-place overwrite of
   the **live, manifest-referenced** object (S3 LWW: B -> A).
3. W1's manifest create then 412-fences. W1 believes it caused no harm. But W2's
   committed snapshot V+1 now resolves stale content A — a **torn / corrupted committed
   read visible to readers**, produced *after* a clean commit.

This is the **same root cause as BC-1/F-A** (unfenced zombie-writer object operations the
model abstracts away): F-A is the DELETE variant (GC removes a live object), DA-1 is the
PUT variant (a stale write overwrites a live object). Both are split-brain on the *object*
layer, squarely my primary lane. **DA-1 is on the commit data path**, so it is at least as
gating as BC-1.

**Why the TLA+ model is blind to it (the dangerous part):** `ObjId(v,k) == ToString(v) o
"-" o ToString(k)` and `StagedObjs(v) == {ObjId(v,1)}` — a data object's identity is a pure
function of `(version, shard)`. Two writers staging "the same" object add the **identical
set element** to `dataObjects`; the union is idempotent. The model has **no notion of
content changing under a stable id**, so `NoTornCommit` (`objs \subseteq dataObjects`)
passes vacuously w.r.t. this attack. The model proves write-once immutability that the
ADR's key schema does not realize — a safety-critical model<->implementation divergence
(formal-verification-policy: "drift = a bug").

## BC-4 — binding condition (must be discharged before commit-path tasks land)

**BC-4 (data-object key uniqueness — owner T-0010 commit writer + T-0046 ADR/model fix;
verified by T-0038 model refinement + a BC-2-class mock test).** The fix is a key-naming
constraint, **not** a redesign — the manifest-CAS, fencing, durability barrier, pinning,
GC, and attach modes are unchanged.

1. **Data-object keys must be unique per write attempt** so a fenced/zombie writer can
   never address a key a winning manifest references. Preferred: **content-addressed**
   (`db/data/<content-hash>/<shard>.col`) — write-once becomes physically true, identical
   content dedupes, a stale writer's different content lands on a different key, and data
   PUTs may use the create-only `If-None-Match:*` precondition for defence in depth.
   Alternative: **writer-epoch/attempt-scoped** (`db/data/v<V+1>/<epoch-or-uuid>/<shard>.col`),
   with the manifest recording the exact keys it references.
2. **State the data-write precondition:** no two distinct write attempts can mutate the
   same key; the durability barrier is over immutable, attempt-unique objects.
3. **Fix the orphan/GC story** (ADR §2 step 2, §6.4): a fenced writer's staged objects are
   genuine orphans only under a distinct (hash/epoch) key never referenced by a committed
   manifest; restate §6.4 orphan identification accordingly.
4. **Refine the TLA+ model** so a staged object's identity depends on the writer/attempt
   (e.g. `ObjId(v, w, k)` or a per-write token) -> two writers racing version v stage
   **distinct** ids; the winner's manifest references only its own; add
   `OrphansNeverReferenced` (folds into formal-methods condition C-A / T-0038) and re-run
   the checker. This converts write-once immutability from an assertion into a checked
   property. Coordinate with `steering-storage` (data-key layout / SPIKE-0003 cross-version
   sharing — content-addressing dovetails with their ref-counted-GC constraint) and
   `steering-formal-methods` (model re-check).

**Gate status of BC-4:** hard pre-`ready`/land-gate for the commit-writer (T-0010) and
GC/lease tasks (T-0012/T-0026), and a named model-refinement obligation (T-0038) — exactly
the disposition of BC-1. Until BC-4's ADR+model fix lands and re-checks clean, the
implemented data path is **not** covered by the ratified model, and T-0010/T-0026 must not
become `ready`.

## Disposition (board-honest, given the SPIKE-0002 revert)

- **SPIKE-0005 -> `done`.** Its three chartered constraints (C1/C2/C3) are surfaced AND
  faithfully realized in the SPIKE-0002 ADR + TLA+ model (on the `work/SPIKE-0002-...`
  branch; confirmed by my pass and by formal-methods decision 0014). DA-1/BC-4 is a
  *newly surfaced* pre-ratification constraint; per the constraints-rider charter these
  obligations are "non-blocking for the SPIKE itself; blocking for commit-path
  implementation readiness." SPIKE-0005's deliverable — surface + specify the
  pre-ratification constraints and confirm SPIKE-0002 addresses them — is complete for
  C1/C2/C3 and now also *records* C4/DA-1 for SPIKE-0002 to discharge. Leaving SPIKE-0005
  open would falsely block a rider whose chartered job is done.
- **SPIKE-0002 design gate is UNRATIFIED on the working tree** (the peer land was
  reverted; artifacts only on the work branch). I am **not** recording a SPIKE-0002
  primary ratification here. **DA-1 / Constraint 4 is now a pre-ratification obligation
  on SPIKE-0002**: when the SPIKE-0002 ADR + model are (re-)submitted for my primary
  ratification, they MUST additionally discharge Constraint 4 (per-write-attempt-unique
  data-object keys + the model refinement) or I will withhold the primary sign-off.
- **No implementation task is `ready`.** The commit-path tasks (T-0010/T-0011/T-0012/
  T-0013/T-0026/T-0038) depend on SPIKE-0002, which is `in_review`. The prove-before-code
  gate stays closed, correctly. The hard pre-`ready`/land-gates for commit-path code are
  the mock-fidelity test (Constraint 1 / BC-2-class), the zombie integration test, and
  now Constraint 4's data-key uniqueness + model re-check.
- **Follow-up task T-0046** filed: discharge Constraint 4 / DA-1 inside the SPIKE-0002
  ADR + TLA+ model (data-object key uniqueness + model refinement giving racing writers
  distinct staged ids + `OrphansNeverReferenced`), routed to the SPIKE-0002 author and
  back to me + `steering-formal-methods` + `steering-storage` for a fast confirm. This is
  a dependency the SPIKE-0002 ratification must clear.

## Why this disposition (not a unilateral block, not a silent agree)

A unilateral CHANGES_REQUESTED would contradict an already-recorded peer primary
ratification of a genuinely sound protocol and stall Cat. 1/7/11 GATE progress (combined
weight 22) against the wallclock — the wrong trade. A silent agreement would let DA-1 — a
real torn committed read — ship, which "ACID is non-negotiable" forbids. The correct
disposition is the same one the peer used for the same class of bug (F-A/BC-1): ratify the
sound core, bind the zombie-object-operation fix to the exact implementing tasks as a hard
land-gate, and refine the model so the abstraction is faithful. BC-4 sits beside BC-1 as
the PUT-overwrite twin of the DELETE-removal finding. Both must be discharged before the
data/GC paths land; neither falsifies the design.

**Signed:** steering-distributed-acid (PRIMARY) — T0+~1:50
