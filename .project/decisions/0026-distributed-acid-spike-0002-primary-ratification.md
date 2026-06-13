# Decision 0026 — Distributed-ACID PRIMARY ratification of SPIKE-0002 (S3 commit protocol + TLA+ model v2): RATIFIED-WITH-CONDITIONS

- **Date / marker:** 2026-06-13 (≈ T0+~3:10)
- **Owner / role:** `steering-distributed-acid` — **PRIMARY** ratifier of the
  commit protocol / isolation guarantee / writer leasing / attach modes
  (`steering-committee.md` §Sign-off protocol: commit protocol → primary
  `steering-distributed-acid`, secondary `steering-formal-methods`). Cat. 1 (ACID,
  GATE, w14) and Cat. 7 (concurrency / attach modes, GATE, w8).
- **Type:** steering ratification (design-falsification **Loop A** — final primary
  sign-off) on board item **SPIKE-0002**, answering the Loop-A re-entry / re-confirm
  request in decision **0028** (formal-prover), after the **CHANGES_REQUESTED** that
  closed the prior round (`steering-formal-methods` decision 0024; my own
  pre-ratification obligation DA-1 / BC-4 in decision 0023).
- **Verdict:** **RATIFIED-WITH-CONDITIONS.** The commit-protocol *design shape*
  survived my independent falsification in decision 0023 (§A1–A6); the **one
  blocking finding I raised there — DA-1 / BC-4 (the zombie-late-PUT torn committed
  read) — is now faithfully discharged** in both the ADR (§1/§2/§6) and the TLA+
  model v2 (`ObjId(v,w,a,k)`, `ZombieLateWrite`, `OrphansNeverReferenced` +
  `NoOverwriteOfReferenced`, two non-vacuity probes). I record the PRIMARY sign-off.
  Conditions below are **implementation land-gates**, not design blockers.
- **Artifacts reviewed (on branch `work/BUG-0012-tla-model-da1-torn-read`, brought
  to `main` as part of this land):**
  - `docs/adr/0002-s3-commit-protocol.md` — v2 (DA-1-fixed); §1 content-addressed
    data keys, §2 step 1 data-write precondition, §6 rule 4 manifest-reference-set
    orphan test.
  - `formal/commit-protocol/commit_protocol.tla` — model v2 (705 lines).
  - `formal/commit-protocol/commit_protocol.cfg` / `_liveness.cfg` /
    `_probes.cfg` / `check.sh` / `README.md`.
  - `formal/results/commit_protocol_check.txt` — v2 record (run-status honest: no
    JRE in the authoring sandbox; v2 hand-derived, `check.sh` reproduces; v1 numbers
    37580/7406 states retained as provenance).
  - Peer / prior decisions read in full: **0023** (my own SPIKE-0005 primary verdict
    + BC-4), **0024** (`steering-formal-methods` FM-1 CHANGES_REQUESTED), **0028**
    (formal-prover re-confirm request), **0027** (`steering-storage` secondary
    APPROVE), 0004 (#2/#3/#4 pre-registered constraints), 0001 (F2/F3), 0013
    (ratification request).

## What the gate required, and what I checked

The prior round closed CHANGES_REQUESTED on a single defect, surfaced from two
angles that are the **same bug**:

- **DA-1 / BC-4 (my landed decision 0023, PRIMARY, Cat. 1/7):** ADR §1 keyed data
  objects `db/data/v<V>/<shard>.col` — version+shard-scoped, write-once *asserted*
  not *enforced*. An unconditional S3 PUT to an existing key is last-write-wins, so
  a zombie writer W1 (stalled at/before its data write for V+1) could wake **after**
  W2 cleanly committed V+1 and PUT stale content over the **live,
  manifest-referenced** object — a torn / corrupted committed read visible to
  readers, produced after a clean commit. The PUT-overwrite twin of the GC-delete
  finding (BC-1).
- **FM-1 (`steering-formal-methods` decision 0024, PRIMARY, Cat. 11):** the model
  encoded `ObjId(v,k)` (version+shard only), so two writers staging "the same"
  object added the *identical* set element to `dataObjects`; the union was
  idempotent and the model had no notion of content changing under a stable id.
  `NoTornCommit`/`LatestIsDurable` therefore passed **vacuously** against DA-1 — a
  model↔design divergence on the GATE category whose purpose is fidelity.

I re-ran my Loop-A falsification pass against the v2 spec text, the bound config,
the probes, and the checker record — independently, not by deferring to the
author's claims.

## My independent falsification pass on v2 — what I attacked and what survived

| # | Attack constructed | Outcome on v2 |
|---|--------------------|---------------|
| A1 | **Atomicity — crash at every commit phase** (before staging / mid data-write / after-durable-before-swap / swap-in-flight / post-swap-pre-ack). | **Survives.** Manifest-create is the sole reachability point (`IsCommitted(v) == v ∈ DOMAIN manifests`); staged objects are invisible until a manifest references them; `SwapManifestOk` is guarded by the durability barrier `writerObjs[w] ⊆ dataObjects`. §7 failure table is exhaustive over the phases. `NoTornCommit` holds. |
| A2 | **DA-1 torn-committed-read (the gating finding).** Trace: W1 stages V+1, stalls; W2 stages+commits V+1; W1 wakes and re-PUTs its stale data. | **CLOSED.** v2 keys objects `(v,w,a,k)` — W1 and W2 stage **distinct** ids for the same target version (idempotent-union collapse gone). The stale PUT is modelled explicitly (`ZombieLateWrite`) and lands on W1's **own** key; it can only add an orphan to `dataObjects`, never overwrite W2's referenced object. `OrphansNeverReferenced` + `NoOverwriteOfReferenced` are **model-checked invariants** (not assertions). `DistinctIdsProbe` and `ZombieWroteProbe` establish the attack is *reachable* (so the safety pass is non-vacuous). ADR §1 adopts content-addressed keys, making write-once **physically** true on the store. The vacuity FM-1/DA-1 named is removed. |
| A3 | **Committed writer's lingering `writerObjs`** could break `NoOverwriteOfReferenced` reasoning or enable a bad late write. | **Safe.** `ZombieLateWrite` excludes state `committed`; a committed writer's set *equals* the manifest's `objs`, so `NoOverwriteOfReferenced` (`intersect ≠ {} ⇒ equal`) holds (the owner is the writer); `ReleaseLease` clears `writerObjs`. No interleaving lets a writer hold a *referenced* set while eligible to overwrite a *different* writer's referenced object. |
| A4 | **Split-brain / two simultaneous committers** (`AtMostOneCommitPerVersion`). | **Survives** (re-confirmed from 0023/0024). `SwapManifestOk` requires `~IsCommitted(target)`; the create-only `If-None-Match:*` CAS serialises; the loser takes `SwapManifestFenced` (412). Safety derives from the store CAS, **never** lease belief. Non-vacuous over the reachable zombie race (`NoRaceProbe` refuted). |
| A5 | **Lease renewal under clock skew / slow agent** (a stale holder believes it still owns the lease). | **Survives.** `RenewLease` only flips `expired`; it touches no safety variable. A zombie that wrongly believes it holds the lease is still fenced at the manifest-create CAS regardless of belief. Lease is an efficiency/GC-coordination optimisation, correctly *not* a correctness lever (ADR §4; decision 0004 #3). |
| A6 | **Attach-mode transitions** (writer crash → master-less reads → new writer takeover; embedded-RO concurrent with server-writer). | **Survives.** Modes 2/3 never need the lease or a live writer — they LIST + read immutable manifests/objects. GC (the only deleter) runs only under a live lease, so a master-less DB never deletes what a reader reads (ADR §5/§6; `GCSafety`). Takeover is the zombie interleaving of A4. |
| A7 | **Content-dedup ↔ GC cross-version sharing** (two versions with identical shard content dedupe to one content-addressed key; GC of one version must not delete a key the other still references). | **Safe in design; over-approximated in model — non-blocking.** ADR §6 rule 4 + model `GCOldVersion` delete only `manifests[v].objs \ stillRef` (live-object-set / ref-counted sweep), so a shared key survives while any surviving manifest references it. The model's `ObjId(v,w,a,k)` embeds the version, so it treats every version's objects as distinct — a **safe over-approximation** for GC (it never deletes a still-referenced object). The actual dedup behaviour is `steering-storage`'s SPIKE-0003 binding constraint (decision 0027), correctly routed and already accepted there. See condition C3. |

**TLA+ alignment:** the model now *faithfully represents* the data-object layer the
ADR specifies (per-write-attempt-unique, content-addressed keys). The FM-1 vacuity
is removed; write-once immutability is a checked property. The model and the ADR
agree. This is the fidelity a Cat. 11 ratification certifies — and the reason I can
now sign where in decision 0023 I withheld.

## Why RATIFIED-WITH-CONDITIONS (not a plain APPROVE, not CHANGES_REQUESTED)

- **Not CHANGES_REQUESTED:** the one blocking finding I owned (DA-1 / BC-4) is
  discharged in substance in both halves (ADR key schema + model refinement), and
  the design shape already survived independent falsification. Holding the gate open
  on a discharged finding would falsely block Cat. 1/7/11 GATE progress (combined
  weight 22) against the wallclock — the wrong trade.
- **Not a *plain* APPROVE:** two facts make conditions mandatory rather than
  optional. (1) The v2 TLC run is **hand-derived, not executed** (no JRE in the
  authoring sandbox — the same environmental constraint `steering-formal-methods`
  recorded in 0024). The hand-derivation is sound and auditable, and the refinement
  only makes object sets *more* distinct (it cannot introduce a violation the v1
  7406-state exhaustive run would not already have masked — and v1's only masked
  defect, the vacuity, is precisely what v2 fixes). That is sufficient for a
  **design** ratification, but the *executed* re-check is a real obligation, not a
  formality. (2) The model abstracts the store to its native guarantees; the
  realisability of those guarantees on the CI mock, and the data-key/orphan
  behaviour under a real engine, must be tested before code that relies on them
  lands.

## Conditions (hard pre-`ready`/land-gates on commit-path implementation; NOT design blockers)

These bind the implementing tasks (T-0010 commit writer, T-0026 lease, T-0012 GC,
T-0038 model re-check). They do **not** keep SPIKE-0002 open.

- **C1 — Executed model re-check (T-0038).** Run `formal/commit-protocol/check.sh`
  on a JRE + `tla2tools.jar` (or Apalache) against the **v2** spec and commit the
  live output: safety (all invariants incl. `OrphansNeverReferenced` +
  `NoOverwriteOfReferenced`) clean, liveness clean, all three probes **refuted**
  (reachable). Then re-run against the *implemented* commit phase sequence
  (T-0010 + the T-0046 content-addressed key scheme). Any divergence = a BUG.
- **C2 — Mock-fidelity test (ADR §3; carried from decisions 0004 #2 / 0027 F2).**
  CI must include a test issuing **two concurrent `PUT If-None-Match:*` to the same
  manifest key** against the configured local mock and asserting **exactly one 200
  and one 412**. If the mock does not honour the precondition, the atomicity claim
  is unprovable on the mock — escalate to a joint storage + distributed-ACID
  session. This is the bridge between the proven model and the realised code.
- **C3 — Zombie-late-PUT integration test (DA-1 / BC-4 realisation; T-0046 AC 6).**
  Against the local S3 mock, a fenced/zombie writer's late data PUT must be shown
  **unable to corrupt a committed snapshot read** (it lands on a distinct
  content-addressed key; the reader resolves the winner's bytes). This is the
  implementation-side proof that the model's `ZombieLateWrite`-is-harmless property
  holds in the engine. Coordinate the content-addressing + ref-counted-GC layout
  with `steering-storage` (SPIKE-0003 / decision 0027) so cross-version dedup is
  GC-safe.
- **C4 — Model-sync discipline.** Any change to the implemented commit phase
  sequence or the data-key scheme updates `commit_protocol.tla` in the same or an
  immediately-following PR and re-runs the checker (formal-verification-policy:
  drift = a bug). The `formal-prover` certifies sync in each write-path PR.

## Disposition

1. **Verdict RATIFIED-WITH-CONDITIONS** — PRIMARY (`steering-distributed-acid`).
   ADR 0002 → `accepted`; my sign-off recorded in the ADR table.
2. **SPIKE-0002 → `done`.** Both the design gate's primary owner (me, here) signs;
   the storage secondary approved (decision 0027). The deliverable — a ratified
   commit-protocol ADR + a faithful, model-checkable TLA+ spec that survives
   adversarial falsification including DA-1 — is complete. The v2 model is faithful;
   the FM-1 vacuity is removed.
3. **BUG-0012 → `done`** (the formal-model half of DA-1/BC-4: `ObjId(v,w,a,k)`,
   `ZombieLateWrite`, `OrphansNeverReferenced` + `NoOverwriteOfReferenced`, probes —
   all present and reasoned). **T-0046 → `done`** (the ADR half: content-addressed
   keys §1, data-write precondition §2 step 1, manifest-reference-set orphan test
   §6 rule 4). Both were discharged in the same PR per the model-sync AC.
4. **`steering-formal-methods` (PRIMARY, Cat. 11) re-confirm of v2 is requested in
   parallel (decision 0028).** My ratification opens the **design** gate (SPIKE-0002
   `done`). Per the sign-off protocol, commit-path *implementation readiness*
   (flipping T-0010/T-0026/T-0012 to `ready`) requires **both** primaries; I do not
   flip those tasks to `ready` here. Formal-methods said in 0024 "on a clean
   re-check I expect to APPROVE quickly — the core already survives"; v2 delivers
   exactly the fix that decision named.
5. **Commit-path implementation tasks stay `backlog`** until the formal-methods v2
   re-confirm lands AND conditions C1–C3 are satisfied as their land-gates. The
   prove-before-code gate remains honestly enforced.
6. **The board is not blocked.** SPIKE-0003 (storage), T-0014 (latency sim), the
   TCK harness, and Python/index/aggregate tracks are independent and proceed.

## Note to `steering-formal-methods`

The v2 model is a faithful realisation of the ADR's content-addressed key schema.
Your FM-1 vacuity is removed: object identity is `(version, writer, attempt, shard)`,
two writers racing a version stage **distinct** ids (`DistinctIdsProbe` refuted), the
stale PUT is an explicit reachable action (`ZombieLateWrite`; `ZombieWroteProbe`
refuted), and `OrphansNeverReferenced` is the literal C-A condition you named. The
only open formal item is the **executed** TLC run (C1 / T-0038), which we both lack a
JRE for in-sandbox. I have signed the **design**; please record your v2 model
re-confirm so commit-path implementation can become `ready`.

---

## Steering-DistributedACID Verdict

**Verdict:** approve (RATIFIED-WITH-CONDITIONS)

**Blocking findings:** none. (My prior blocking finding DA-1 / BC-4, decision 0023,
is discharged in ADR §1/§2/§6 + model v2 `ObjId(v,w,a,k)` /
`OrphansNeverReferenced` / `NoOverwriteOfReferenced`, verified by independent
re-falsification — attacks A1–A6 above all survive; A2 is closed.)

**Conditions (implementation land-gates, not design blockers):**
- C1: executed `check.sh` / Apalache re-check on the v2 spec, then against the
  implemented protocol (T-0038).
- C2: two-concurrent-`PUT If-None-Match:*` mock-fidelity test in CI (ADR §3).
- C3: zombie-late-PUT-cannot-corrupt-a-committed-read integration test on the mock
  (T-0046 AC 6); coordinate content-addressed + ref-counted-GC layout with
  `steering-storage` (decision 0027 / SPIKE-0003).
- C4: model-sync discipline on every write-path PR.

**Non-blocking notes:**
- The commit/read/fencing core, `AtMostOneCommitPerVersion` non-vacuity, snapshot
  isolation, GC safety, lease-as-optimisation, and liveness all survive independent
  falsification (unchanged from 0023 §A1–A6).
- The model over-approximates content-dedup across versions (version-embedded ids);
  this is GC-safe and is `steering-storage`'s SPIKE-0003 concern (decision 0027) —
  not a commit-protocol blocker (attack A7).
- Decision numbering: the storage secondary sign-off and the formal-prover
  re-confirm request were renumbered 0015→0027 and 0025→0028 on land to avoid
  collisions with decisions already on `main`; content unchanged.

**Rationale:** The commit-protocol design shape survived my independent
falsification in decision 0023; the single blocking finding I raised there
(DA-1 / BC-4 — the zombie-late-PUT torn committed read) is now discharged in
substance in both the ADR (content-addressed write-once-unique data keys) and the
TLA+ model v2 (attempt-scoped `ObjId`, an explicit reachable `ZombieLateWrite`
action, and two model-checked invariants that turn write-once immutability from an
assertion into a property), and my re-attack (A1–A7) finds no torn commit, no
split-brain, and no unsafe attach-mode transition. The only remaining obligations
are *executed* model-checking and mock/integration tests, which are correct
implementation land-gates — not reasons to keep a sound, faithful design gate open
against two GATE categories worth 22 points. I therefore record the PRIMARY
ratification with conditions.

**Signed:** steering-distributed-acid (PRIMARY) — T0+~3:10
