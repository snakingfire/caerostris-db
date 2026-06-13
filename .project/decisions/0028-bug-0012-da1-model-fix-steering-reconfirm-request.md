# Decision 0028 — BUG-0012 / T-0046 DA-1 fix landed in the SPIKE-0002 model + ADR; request steering re-confirm (Loop A re-entry)

> **Renumbered 0025 → 0028 on land** (steering-distributed-acid, SPIKE-0002
> ratification, decision 0026): `0025` was already taken on `main` by
> `0025-spike-0004-statistics-contract-signoff-request.md` (a different decision).
> This is the `formal-prover` Loop-A re-entry / re-confirm request authored on the
> BUG-0012 work branch and brought to `main` as part of the SPIKE-0002 ratification
> land. Content unchanged from the branch original; only the file number is
> corrected. (My primary ratification answering this request is decision 0026;
> `steering-formal-methods`'s re-confirm is requested in parallel.)

- **Date / marker:** 2026-06-13 (≈ T0+~2:30)
- **Owner / role:** `formal-prover` (author of the TLA+ commit-protocol model and
  the latency cost-model/sim; Cat. 11)
- **Type:** design-falsification **Loop A re-entry** + steering sign-off **request**.
  This is the author's response to the two blocking findings on SPIKE-0002:
  `steering-distributed-acid`'s DA-1 / BC-4 (decision 0023) and
  `steering-formal-methods`'s FM-1 (decision 0024, CHANGES_REQUESTED). It is a
  REQUEST for re-ratification, not a self-ratification.
- **Board items:** **BUG-0012** (formal-model half — this PR) and **T-0046**
  (ADR §1/§2/§6 key-naming half — discharged in the SAME PR per the BUG-0012
  model-sync AC). SPIKE-0002 stays `in_review`.
- **Rubric:** Cat. 11 (formal, GATE, w6 — model fidelity), Cat. 1 (ACID, GATE,
  w14 — "behaviour matches the TLA+ model"), Cat. 7 (GATE, w8).
- **Artifacts in this PR (branch `work/BUG-0012-tla-model-da1-torn-read`, based on
  the SPIKE-0002 branch tip):**
  - `formal/commit-protocol/commit_protocol.tla` — model v2 (DA-1 refinement).
  - `formal/commit-protocol/commit_protocol.cfg` — adds the two new invariants.
  - `formal/commit-protocol/commit_protocol_probes.cfg` — NEW non-vacuity probes.
  - `formal/commit-protocol/check.sh` — runs SANY + safety + liveness + probes.
  - `formal/commit-protocol/README.md` — documents the refinement.
  - `formal/results/commit_protocol_check.txt` — regenerated record (v2).
  - `docs/adr/0002-s3-commit-protocol.md` — §1/§2/§6 + sign-off table updated.

## What the two blocking findings required

**DA-1 / BC-4 (decision 0023) and FM-1 (decision 0024)** are the same defect from
two angles: the model encoded a data object's identity as `ObjId(v, k)` — a pure
function of `(version, shard)`. Two writers racing the same version `V+1` staged
the *identical* set element, so `dataObjects ∪ writerObjs` was idempotent and the
model had **no notion of content changing under a stable id**. Therefore
`NoTornCommit` (`objs ⊆ dataObjects`) and `LatestIsDurable` held **vacuously**
against the DA-1 attack (a zombie writer PUTting stale content over a live,
committed object's version-scoped key — S3 last-write-wins). The committed
checker record said "NoTornCommit holds" *because the model was blind to the
tear*, not because the design was torn-read-free. A Cat. 11 ratification certifies
*fidelity*; the model was not faithful, so the verdict was CHANGES_REQUESTED.

## How this PR discharges them (the fix is a key-naming + model-fidelity fix,
## NOT a redesign — the manifest-CAS / fencing / durability-barrier / pinning /
## GC / attach-mode design is unchanged)

### TLA+ model (BUG-0012 acceptance criteria)

1. **`ObjId` / `StagedObjs` refined to depend on the writer/attempt.** New
   encoding `ObjId(v, w, a, k)` keyed by `(version, writer, ATTEMPT, shard)`,
   modelling the ADR's content-addressed key. Added a monotone per-writer
   `writerAttempt` counter (bumped on every `AcquireLease`). **Two writers racing
   version `v` now stage DISTINCT ids; the winner's manifest references only its
   own object set.** The idempotent-union collapse is gone.  *(AC line 1 ✓)*
2. **`OrphansNeverReferenced` invariant added** (a fenced/crashed writer's staged
   objects are never in any committed manifest's `objs`) **and**
   **`NoOverwriteOfReferenced`** (no committed-manifest-referenced id is owned by
   any other writer/attempt — no last-write-wins tear of a live object). Both
   added to `commit_protocol.cfg`'s INVARIANT list and to `SafetyInvariant`.
   *(AC line 2 ✓ — exceeds it: two invariants, one being the literal
   `OrphansNeverReferenced` C-A condition from decisions 0014/0024.)*
3. **Non-vacuity probes added** (`commit_protocol_probes.cfg`): `DistinctIdsProbe`
   (two writers racing the same version now hold **distinct** staged ids — the
   exact collapse v1 hid) and `ZombieWroteProbe` (a fenced writer durably holds
   its orphan alongside the committed snapshot — the `ZombieLateWrite` action that
   makes the stale-PUT attack representable is exercised). Each is EXPECTED to be
   REFUTED (= the behaviour is reachable, so the safety pass is non-vacuous).
   *(AC line 3 ✓)*
4. **Checker record regenerated** (`formal/results/commit_protocol_check.txt`).
   *(AC line 4 — see "Run-status honesty" below.)*
5. **Paired with the ADR §1/§2/§6.4 key-naming fix (T-0046) in the SAME PR.**
   *(AC line 5 ✓.)*
6. **Routed to steering for re-confirm via this decision.**  *(AC line 6 — this
   request.)*

### ADR 0002 (T-0046 acceptance criteria)

- **§1** object-layout table: data keys are now **content-addressed**
  (`db/data/<content-hash>/<shard>.col`), unique per write attempt; the manifest
  records the **exact** keys it references; the attempt-scoped alternative is
  documented. *(T-0046 AC 1 ✓)*
- **§2 step 1**: states the **data-write precondition** — no two distinct write
  attempts can mutate the same key; the durability barrier is over immutable,
  attempt-unique objects. *(T-0046 AC 2 ✓)*
- **§6 rule 4**: orphan story corrected — a fenced writer's objects are genuine
  orphans under a distinct content key never referenced by a committed manifest;
  GC identifies orphans by the **manifest-reference-set** test (ref-counted /
  live-object-set sweep), replacing the old `v<V>/`-prefix test. *(T-0046 AC 3 ✓)*
- The §6 storage binding-constraint note (decision 0015) is reconciled:
  content-addressing + ref-counted GC **directly satisfies** the
  cross-version-sharing constraint `steering-storage` anticipated.
- Integration test (zombie late PUT cannot corrupt a committed read on the mock):
  **deferred to the commit-writer task (T-0010)** when the engine + S3 mock exist;
  noted in T-0046 as an implementation land-gate. *(T-0046 AC 6 — implementation
  phase.)*

## Run-status honesty (this is load-bearing for a Cat. 11 record)

This authoring sandbox has **no Java runtime** (`/usr/bin/java` → "Unable to
locate a Java Runtime") and no `tla2tools.jar` — the **same constraint**
`steering-formal-methods` recorded in decision 0024 ("could not re-run TLC … the
DA-1 vacuity finding is established by reading the model, not by re-running") and
that T-0046's notes assign to "a runner with a JRE". I therefore did **not**
fabricate a TLC run. The regenerated `commit_protocol_check.txt`:

- States the run status truthfully (no JRE here; reproduce via `./check.sh`).
- Gives a **rigorous hand-derivation** of why each invariant holds and each probe
  is refuted (singleton object sets are equal-or-disjoint; winner ≠ loser ⇒
  disjoint ids; `ZombieLateWrite` is monotone on `dataObjects` so preserves every
  `⊆` invariant; liveness unaffected because `ZombieLateWrite` never disables the
  swap and becomes a stutter once the orphan is durable).
- Retains the v1 executed numbers (37580/7406 states) as provenance, with the
  explicit caveat that v1's `NoTornCommit` was vacuous — the bug this fixes.

The model is small (singleton object sets per writer/version; bound
W={w1,w2}/R={r1,r2}/MaxVersion=2/MaxLeaseEpoch=3) and `./check.sh` mechanically
re-checks it on any JRE; **T-0038** runs Apalache against the *implemented*
protocol (T-0010 commit writer + the T-0046 content-addressed key scheme).

## Request to steering (Loop A re-entry)

- **`steering-formal-methods`** (primary, Cat. 11): please re-run your
  falsification pass against model v2. The specific question your FM-1 raised —
  "is `NoTornCommit` non-vacuous against DA-1?" — is now answered structurally:
  `DistinctIdsProbe` shows racing writers stage distinct ids, and
  `OrphansNeverReferenced` / `NoOverwriteOfReferenced` are checked properties, not
  assertions. Confirm the C-A condition (`OrphansNeverReferenced` + writer/
  attempt-scoped id) is discharged. If you have a JRE, `./check.sh` produces live
  numbers; if not, the hand-derivation is auditable line-by-line.
- **`steering-distributed-acid`** (primary, Cat. 1/7): please confirm BC-4 is
  discharged in ADR §1/§2/§6 (content-addressed, write-once-unique data keys; the
  data-write precondition; the orphan = not-in-any-live-manifest test) and that
  the model now represents the PUT-overwrite twin of BC-1.
- **`steering-storage`** (secondary): please confirm the content-addressing +
  ref-counted-GC choice satisfies your decision-0015 cross-version-sharing binding
  constraint (it was written anticipating exactly this).

**Until both primaries re-confirm, SPIKE-0002 stays `in_review` and the
commit-path implementation tasks (T-0010 writer, T-0026 lease, T-0012 GC, T-0011/
T-0013/T-0019/T-0021/T-0038) stay `backlog`.** The prove-before-code gate is held
honestly open; nothing is unblocked prematurely, and no independent work
(SPIKE-0003 storage, T-0014 latency sim, TCK harness) is blocked by this.

## Why this disposition

`steering-formal-methods` wrote in decision 0024: "On a clean re-check I expect to
APPROVE quickly — the core already survives." This PR delivers exactly the small,
tracked fix that decision named (content-addressed keys in the ADR;
`ObjId(v,w,k)`-style id + `OrphansNeverReferenced` in the model; re-run the
checker), plus a second structural invariant (`NoOverwriteOfReferenced`) and two
non-vacuity probes that make the re-ratification meaningful rather than a
rubber-stamp. The fix changes only the object-id/key layer; the commit/fencing/
isolation/GC/attach-mode design that survived independent falsification (decisions
0023 §A1–A6, 0024 §"what survived") is untouched.

**Signed:** formal-prover — T0+~2:30 (request; not a ratification)
