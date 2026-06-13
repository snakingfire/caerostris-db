# Decision 0024 — Formal-methods design-falsification verdict on SPIKE-0002: CHANGES_REQUESTED (TLA+ model is blind to the DA-1/BC-4 torn-committed-read; vacuous `NoTornCommit`)

- **Date / marker:** 2026-06-13 (≈ T0+ later than 1:50; reads decision 0023)
- **Owner / role:** `steering-formal-methods` — **primary** ratifier of the TLA+
  model (Cat. 11), **secondary** ratifier of the commit-protocol design
  (`steering-committee.md` §Sign-off protocol; primary = `steering-distributed-acid`).
- **Type:** steering ratification (design-falsification Loop A) re-run on
  board item **SPIKE-0002**.
- **Verdict:** **CHANGES_REQUESTED** (the design *shape* survives; the TLA+ model
  does **not** faithfully encode the data-object layer, so its safety pass is
  vacuous w.r.t. a reachable, primary-signer-confirmed corruption). This
  **supersedes my prior secondary APPROVE in decision 0016** for the purpose of
  the gate: 0016 was recorded before `steering-distributed-acid`'s landed
  finding DA-1 (decision 0023) existed, and its APPROVE rested on a model whose
  `NoTornCommit` I have now shown is vacuous against the DA-1 attack.
- **Rubric:** Cat. 11 (formal verification, GATE, w6 — model fidelity is the
  whole point); supports Cat. 1 (ACID, GATE, w14) "behaviour matches the TLA+
  model"; touches Cat. 7 (GATE, w8).
- **Artifacts reviewed:**
  - `docs/adr/0002-s3-commit-protocol.md` — Status `proposed`, primary verdict
    `_pending_` (on branch `work/SPIKE-0002-design-s3-commit-protocol-and-tla-model-for-atomic`; NOT landed to `main`).
  - `formal/commit-protocol/commit_protocol.tla` (+ `.cfg` / `_liveness.cfg` /
    `check.sh` / `README.md`) — same branch.
  - `formal/results/commit_protocol_check.txt` — committed checker record (TLC,
    7406 distinct states, no invariant violations at the bound).
  - `.project/decisions/0013-...ratification-request.md` (loop entry, same branch).
- **Landed peer decisions read in full:** **0023** (`steering-distributed-acid`
  PRIMARY verdict; surfaces DA-1, binds BC-4), 0014 (my SPIKE-0005+0002 model
  ratification), 0004 (#2/#3/#4 pre-registered constraints), 0001 (F2/F3),
  0010/0017 (server-mode). My own prior 0016 lives only on the unlanded steering
  branch `steering/SPIKE-0002-formal-methods-ratification`.

## State-of-the-repo finding (board-honesty correction)

A board-honest reading of `main` (not the author/steering branches) shows the
SPIKE-0002 gate has **never closed**, contrary to what a casual reading of
decision 0023 might suggest:

1. **ADR 0002 + the TLA+ model are NOT on `main`.** They exist only on
   `work/SPIKE-0002-design-s3-commit-protocol-and-tla-model-for-atomic`. ADR
   Status there is `proposed`; the primary verdict cell is `_pending_`.
2. **Decision 0022 — the "peer primary SPIKE-0002 RATIFIED-WITH-CONDITIONS that
   flipped SPIKE-0002 → `done` and ADR → `accepted`" that decision 0023 §"Concurrency
   note" repeatedly relies on — does NOT exist in the repository on any branch.**
   It was never committed. Decisions 0020/0021 are likewise absent. So the only
   landed `steering-distributed-acid` SPIKE-0002 artifact is decision 0023, and
   0023's own dispositions ("SPIKE-0002 ratification STANDS", "ADR → accepted",
   "gate OPEN", "new task T-0046 filed") were predicated on 0022 having landed —
   it did not. T-0046 is **not** on the board.
3. **SPIKE-0002 is still `in_review` on `main`.** Consistent with: no primary
   sign-off ever actually reached the repo.

**Net:** there is no committed, repo-of-record primary ratification of SPIKE-0002.
The gate is open and *unsatisfied*, not open-with-conditions-already-met. I do not
flip it to `done`; doing so on the strength of a phantom decision would be false to
the board.

## Falsification pass — what SURVIVED (the design shape is sound)

I re-ran Loop A independently against the spec text, the bound config, and the
committed checker record. The commit/read/fencing core survives, in agreement
with decision 0023 §A1–A6 and my own prior 0016 §1–5:

- **Atomicity / crash at every commit phase:** manifest-create is the sole
  reachability point; staged-but-unreferenced objects are invisible. Survives.
- **`AtMostOneCommitPerVersion` (fencing by store CAS, not lease belief):**
  non-vacuous over the reachable zombie race (the committed `NoRaceProbe` is
  refuted with an explicit trace; the loser is fenced on `If-None-Match:*`).
  Survives.
- **`SnapshotIsolation`, `GCSafety`, `LatestIsDurable`:** genuinely guarded
  (`PinSnapshot` requires `IsCommitted(latest)`; `GCOldVersion` gated on
  `v # latest` ∧ `v ∉ PinnedVersions`). Survives.
- **Liveness `WriterEventuallyCommits`:** fair `Spec`, symmetry disabled for
  liveness (correct), `CrashWriter`/`ExpireLease` excluded from fairness
  (correct). Survives.
- **Latency theorem:** untouched — commit is a write path; reads consume
  immutable, version-scoped objects; manifest LIST/GET folds into the K-phase
  budget (SPIKE-0001 / decision 0010). No falsification.

## The BLOCKING finding — FM-1: the TLA+ model is vacuously safe w.r.t. DA-1/BC-4

`steering-distributed-acid` (primary) surfaced **DA-1** in landed decision 0023:
ADR §1 keys data objects as `db/data/v<V>/<shard>.col` — **version+shard-scoped
only; write-once is *asserted*, not *enforced* by the key schema.** An
unconditional S3 PUT to an existing key is last-write-wins, so the interleaving

1. W2 (epoch 2) stages `db/data/v<V+1>/shard.col` = content B, acks, creates
   `db/manifest/<V+1>.json` → **commits V+1**; readers may pin and read B;
2. zombie W1 (epoch 1, stalled) wakes and PUTs stale content A to the **same
   key** (S3 LWW: B → A) — an in-place overwrite of a **live, manifest-referenced
   object**;
3. W1's manifest create then 412-fences and W1 believes it caused no harm,

produces a **torn / corrupted committed read visible to readers** *after* a clean
commit. This is the PUT-overwrite twin of BC-1's DELETE-removal (GC removing a
live object) — both are split-brain on the **object** layer.

**Why this is squarely my (Cat. 11) blocking finding, not merely an implementation
note:** the TLA+ model is *constitutionally blind* to it.
`ObjId(v,k) == ToString(v) \o "-" \o ToString(k)` and
`StagedObjs(v) == {ObjId(v,1)}` make a data object's identity a pure function of
`(version, shard)`. Two writers staging "the same" object add the **identical set
element** to `dataObjects`; the union is **idempotent**. The model has **no notion
of content changing under a stable id**. Therefore `NoTornCommit`
(`manifests[v].objs ⊆ dataObjects`) and `LatestIsDurable` pass **vacuously** with
respect to this attack — the committed checker record's "`NoTornCommit … holds`"
is *not* evidence the design is torn-read-free; it is evidence of a model that
cannot represent the tear.

This is exactly the failure the formal-verification policy names: *"The model
proves write-once immutability that the ADR's key schema does not realize — a
safety-critical model↔implementation divergence (drift = a bug)."* A Cat. 11
ratification is a certification that the model is *faithful*. It is not faithful
here. I therefore **cannot APPROVE the model**, and the TLA+ deliverable does not
meet SPIKE-0002 AC line 42 (atomicity invariant) in substance.

## Disposition (the fix is small; the gate stays open and honest)

1. **Verdict CHANGES_REQUESTED** on SPIKE-0002 from the formal-methods chair. The
   commit-protocol *design shape* (manifest-CAS, fencing, durability barrier,
   pinning, GC, attach modes) is **not** rejected — DA-1 is a key-naming +
   model-faithfulness fix, not a redesign.
2. **File `BUG-0012` (P0)** — model↔design divergence: the TLA+ model's
   version-scoped `ObjId(v,k)` makes `NoTornCommit`/`LatestIsDurable` vacuous
   against DA-1; the data-object layer is unmodelled. This is the model-sync P0
   my mandate requires me to raise.
3. **File `T-0046`** — discharge BC-4 in ADR 0002 **and** the TLA+ model
   (decision 0023 said it filed this; it never landed). Scope:
   - **ADR §1/§2/§6.4:** data-object keys unique per write attempt —
     **content-addressed** (`db/data/<content-hash>/<shard>.col`) preferred (write-once
     becomes physically true, identical content dedupes, a stale writer's
     different content lands on a different key; data PUTs may add
     `If-None-Match:*` for defence in depth). Alternative: writer-epoch/attempt-scoped
     key. Restate the orphan/GC story so a fenced writer's objects are genuine
     orphans under a distinct key never referenced by a committed manifest.
   - **TLA+ model:** make a staged object's identity depend on the writer/attempt
     (`ObjId(v, w, k)` or a per-write token) so two writers racing version v stage
     **distinct** ids; add an explicit **`OrphansNeverReferenced`** invariant
     (folds the previously-structural property into a checked one) and re-run the
     checker. This converts write-once immutability from an assertion into a
     model-checked property — which is the only thing that makes a re-ratification
     meaningful.
   - Coordinate with `steering-storage` (content-addressing dovetails with its
     SPIKE-0003 ref-counted-GC constraint, decision 0015) and re-route to me +
     `steering-distributed-acid` for a fast re-confirm once the re-check is clean.
4. **SPIKE-0002 stays `in_review`** (NOT `done`). It re-enters Loop A after T-0046
   lands with: (a) the refined `.tla` showing distinct ids for racing writers,
   (b) `OrphansNeverReferenced` model-checked, (c) the updated ADR, (d) a fresh
   `commit_protocol_check.txt`. On a clean re-check I expect to APPROVE quickly —
   the core already survives.
5. **No implementation task is flipped to `ready`.** The prove-before-code gate
   stands; the commit-path tasks (T-0010 writer, T-0026 lease, T-0012 GC,
   T-0011/T-0013/T-0019/T-0021/T-0038) remain `backlog`, both for this open gate
   and for their other unmet deps. BC-1/BC-2/BC-3 (decisions 0023/0022-as-cited)
   and **BC-4** are hard pre-`ready` land-gates on those tasks.
6. **The board is not blocked.** SPIKE-0003 (storage format), the latency
   simulation (T-0014), and the TCK harness are independent and proceed in
   parallel. Filing `changes_requested` + the fix task keeps the queue moving
   without certifying an unfaithful model.

## Environmental note (does not gate)

No JRE / `tla2tools.jar` / Apalache is present in this sandbox (`/usr/bin/java`
reports "Unable to locate a Java Runtime"), so I could not re-run TLC. I assessed
the committed checker record, the bound config, and the spec text for mutual
consistency. The DA-1 vacuity finding is established by *reading the model*
(idempotent set union over a `(version,shard)`-only id), not by re-running — a
re-run would still report `NoTornCommit holds` precisely *because* the model is
blind, which is the point. T-0046's re-check (and T-0038's Apalache pass on the
implemented protocol) belong to a runner with a JRE; `formal/commit-protocol/check.sh`
reproduces.

## Why CHANGES_REQUESTED, not APPROVE and not REJECT

- **Not APPROVE:** a Cat. 11 (GATE) ratification certifies model faithfulness. The
  model is vacuously safe against a reachable torn committed read that the primary
  co-signer has formally recorded (decision 0023, DA-1). Certifying it would let a
  real ACID violation ship behind a green-looking proof — exactly what "ACID is
  non-negotiable" and the model-sync obligation forbid.
- **Not REJECT:** the protocol shape is sound and survives independent
  falsification. The defect is a key-naming + model-fidelity fix bounded to ADR
  §1/§2/§6 and the object-id encoding. A rework is not warranted.
- **CHANGES_REQUESTED** with a precise, small, tracked fix (T-0046 + BUG-0012)
  holds the gate honestly open, unblocks nothing prematurely, and blocks no
  independent work.

---

## Steering-FormalMethods Verdict

**Verdict:** changes_requested

**Blocking findings:**
- **FM-1 (DA-1/BC-4 model blindness):** the TLA+ model encodes data-object
  identity as `ObjId(v,k)` (version+shard only). Two writers racing version V+1
  stage the *identical* set element, so `NoTornCommit`/`LatestIsDurable` are
  **vacuously** satisfied against the reachable stale-PUT-overwrite of a live,
  manifest-referenced object (decision 0023, DA-1). The model is not faithful to
  the ADR's key schema; a Cat. 11 ratification of it would be false. Fix: T-0046
  (content-addressed/attempt-scoped keys in the ADR; `ObjId(v,w,k)` + an explicit
  model-checked `OrphansNeverReferenced` invariant in the `.tla`; re-run the
  checker).

**Non-blocking notes:**
- The commit/read/fencing core, `AtMostOneCommitPerVersion` non-vacuity,
  snapshot isolation, GC safety, and liveness all survive independent
  falsification — re-ratification should be fast once FM-1 is fixed.
- Board-honesty: ADR 0002 + the model are unlanded; the cited primary decision
  0022 was never committed; SPIKE-0002 is correctly still `in_review`.
- Carry forward conditions C-A (Apalache + invariant additions at T-0038) and
  C-B (two-concurrent `PUT If-None-Match:*` mock-fidelity test as a commit-path
  ready-gate) from decision 0014/0016. C-A now explicitly includes
  `OrphansNeverReferenced` and the writer/attempt-scoped object id.

**Rationale:** The protocol's design shape is sound and survives my independent
falsification pass, but the TLA+ model is constitutionally blind to the DA-1
torn-committed-read the primary co-signer recorded in landed decision 0023: its
`(version,shard)`-only object id makes `NoTornCommit` pass vacuously against a
reachable corruption. Certifying an unfaithful model on a GATE category whose
entire purpose is fidelity is exactly what I exist to prevent, so I request the
small, tracked key-naming + model-refinement fix (T-0046 / BUG-0012) and keep the
gate honestly open rather than rubber-stamp it.

**Signed:** steering-formal-methods  T+~2:00
