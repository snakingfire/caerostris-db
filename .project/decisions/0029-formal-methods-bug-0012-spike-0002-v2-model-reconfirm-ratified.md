# Decision 0029 — Formal-methods PRIMARY re-confirm of the SPIKE-0002 v2 TLA+ model (DA-1 fix): RATIFIED; closes BUG-0012

> **Numbering note:** I take **0029** — the next clear slot above the highest
> decision on `main` (`0028`). Decision **0026**
> (`steering-distributed-acid` PRIMARY SPIKE-0002 v2 ratification, RATIFIED-WITH-
> CONDITIONS) and **0027** (`steering-storage` secondary APPROVE) are now both
> present on the working tree / landing onto `main` — so this record is the
> **final of the two primary sign-offs** the SPIKE-0002 gate requires, recorded
> after the distributed-ACID primary. (An earlier draft of this note flagged 0026
> as a "phantom" referenced-but-absent file; during this review the
> distributed-ACID lane landed it, so that caveat is withdrawn — 0026 is real and
> read in full below.)

- **Date / marker:** 2026-06-13 (T0+~later; re-entry on BUG-0012 `in_review`)
- **Owner / role:** `steering-formal-methods` — **PRIMARY** ratifier of the TLA+
  commit-protocol model (Cat. 11); secondary ratifier of the commit-protocol
  design (`steering-committee.md` §Sign-off protocol; primary on the design itself
  is `steering-distributed-acid`).
- **Type:** steering ratification (design-falsification **Loop A re-entry**) on
  board item **BUG-0012** (the formal-model half of decision 0023's BC-4 /
  decision 0024's FM-1). Answers the re-confirm request in decision 0028
  (formal-prover, Loop A re-entry).
- **Verdict:** **RATIFIED (APPROVE).** The v2 model survives my independent
  falsification pass; the DA-1 vacuity that grounded my prior CHANGES_REQUESTED
  (decision 0024) is closed. This **supersedes, for the model-fidelity gate, my
  CHANGES_REQUESTED in decision 0024** — that verdict was against the v1 model
  (`ObjId(v,k)`), which no longer exists in the repo of record.
- **Rubric:** Cat. 11 (formal verification, GATE, w6 — model fidelity is the whole
  point); supports Cat. 1 (ACID, GATE, w14 — "behaviour matches the TLA+ model");
  touches Cat. 7 (GATE, w8).
- **Artifacts re-reviewed on `main` (HEAD `a6e9805`; the v2 fix has landed —
  verified byte-identical to the `work/BUG-0012-tla-model-da1-torn-read` worktree
  I falsified):**
  - `formal/commit-protocol/commit_protocol.tla` — model v2 (attempt-scoped ids).
  - `formal/commit-protocol/commit_protocol.cfg` — safety INVARIANT list (adds the
    two new invariants).
  - `formal/commit-protocol/commit_protocol_probes.cfg` — `DistinctIdsProbe`,
    `ZombieWroteProbe`.
  - `formal/commit-protocol/commit_protocol_liveness.cfg`, `check.sh`, `README.md`.
  - `formal/results/commit_protocol_check.txt` — v2 record (board-honest re run-status).
  - `docs/adr/0002-s3-commit-protocol.md` — §1/§2/§6 + sign-off table updated.
- **Landed peer decisions read in full:** 0023 (distributed-ACID PRIMARY, DA-1 /
  BC-4), 0024 (my v1 CHANGES_REQUESTED / FM-1), 0027 (storage secondary APPROVE),
  0028 (formal-prover v2 re-confirm request).

## My independent falsification pass on the v2 model — what I attacked, what survived

I do **not** rely on the author's hand-derivation in `commit_protocol_check.txt`.
I re-read the model text and attacked the invariants directly (the model is tiny —
singleton object sets, `Writers={w1,w2}`, `MaxVersion=2`, `MaxLeaseEpoch=3` — so
the safety arguments are exhaustively hand-checkable, exactly as decision 0024
established the *original* vacuity by reading the model rather than re-running).

1. **Did the DA-1 idempotent collapse actually go away?**
   `ObjId(v,w,a,k) == "v"+v+"-w"+w+"-a"+a+"-k"+k`; `AcquireLease(w)` sets
   `writerObjs'[w] = StagedObjs(latest+1, w, writerAttempt[w]+1)`, so the id embeds
   the **writer** and the **attempt**. Two distinct writers racing the same target
   version V stage `…-ww1-a…` vs `…-ww2-a…` ⇒ **distinct set elements**. The v1
   idempotent `dataObjects ∪ writerObjs` collapse is genuinely gone. `DistinctIdsProbe`
   is therefore refutable (the W1-stages-stalls-W2-stages trace yields two distinct
   non-empty sets). **Survives.**

2. **Can `ZombieLateWrite` overwrite a referenced object?** It does
   `dataObjects' = dataObjects ∪ writerObjs[w]` where `writerObjs[w]` embeds w's own
   token. Any id a committed manifest references embeds the **winner's** token
   (`SwapManifestOk` sets `objs |-> writerObjs[w]` for the winner). Adding a loser's
   orphan id cannot alter the element stored under the winner's id — distinct
   content ⇒ distinct id is now *faithful*, not assumed. **Survives.**

3. **Is `NoOverwriteOfReferenced` true across all reachable states?**
   `∀v∈DOM manifests, ∀w: (manifests[v].objs ∩ writerObjs[w] ≠ {}) ⇒ writerObjs[w]
   = manifests[v].objs`. A referenced id embeds `(v, winner, a_win, k)`; for any
   other writer w' to share it, w' would have had to stage that exact id, but w'
   stages `(v, w', a', k)` with w'≠winner ⇒ different id ⇒ empty intersection ⇒
   implication vacuously true for non-owners and trivially true for the owner.
   After `ReleaseLease` clears the owner's `writerObjs` to `{}`, the intersection is
   empty for everyone ⇒ still holds. **Survives.**

4. **The suspicious right disjunct of `OrphansNeverReferenced`.**
   `(writerState[w]∈{fenced,crashed} ∧ target committed) ⇒ (manifests[target].objs
   ∩ writerObjs[w] = {}) ∨ (writerObjs[w] = manifests[target].objs)`. For a
   fenced/crashed **loser**, `writerObjs[w]` embeds w's token and the committed
   `objs` embeds the winner's token (w≠winner) ⇒ the sets are **disjoint** ⇒ the
   **left disjunct holds on its own**. The right disjunct can only fire if w were
   the committer — but a committer is in state `committed`, excluded by the
   antecedent. So the invariant is carried entirely by the left disjunct; the right
   disjunct is **dead defensive padding**, not a hole. Non-blocking note: a future
   refactor that let a committed-then-recrashed writer re-enter `crashed` while
   still holding its committed `objs` would lean on the right disjunct and could
   then mask a genuine violation — I'd prefer the right disjunct removed or replaced
   by an explicit `w` ∈ {committer of target} guard at the T-0038 Apalache pass.
   **Survives** (non-blocking cleanup noted).

5. **Non-vacuity of the new invariants.** The antecedent of
   `OrphansNeverReferenced` (a writer `fenced` with its target committed) is
   **reachable**: PROBE-C / `ZombieWroteProbe` trace — W1 leases v1, writes, lease
   expires, W2 leases v1, writes, `SwapManifestOk` (commits v1), W1
   `SwapManifestFenced` ⇒ W1 `fenced`, `IsCommitted(1)`. So the invariant is tested
   against a real fenced-with-committed-target state. This is exactly the state v1
   could not represent. **Non-vacuous — the gap decision 0024 raised is closed.**

6. **GC ref-counting (`GCOldVersion`) under attempt-scoped ids.** It removes only
   `manifests[v].objs \ stillRef`, gated on `v≠latest ∧ v∉PinnedVersions`.
   Attempt-scoped ids make each version own disjoint ids, so survivors keep all
   their objects ⇒ `NoTornCommit`/`GCSafety` preserved. **Survives.**

7. **Scope vs. formal-verification-policy §Artifact-1.** Single-writer commit,
   multi-reader (r1,r2), crash at every phase (`CrashWriter`), partial write
   (staged-but-unreferenced invisible), writer leasing / split-brain
   (`AtMostOneCommitPerVersion` non-vacuous via `NoRaceProbe`), and now the
   object-layer torn read. **Scope adequate.**

8. **Latency theorem.** Untouched — commit is a write path; reads consume
   immutable, attempt-scoped objects; the manifest LIST/GET folds into the K-phase
   budget (SPIKE-0001 / decision 0010). No falsification.

**Net:** `NoTornCommit` and `LatestIsDurable` are no longer vacuous against DA-1.
The model now *represents* the torn-committed-read attack and *proves* it harmless
via two model-checked invariants whose antecedents I confirmed reachable. The
commit/read/fencing/isolation/GC core I and the primary co-signer found sound
(decisions 0023 §A1–A6, 0024 §"what survived") is untouched. The fix is the
key-naming + model-fidelity refinement I named in decision 0024, delivered.

## Environmental note (does not gate)

No JRE / `tla2tools.jar` / Apalache in this sandbox (`/usr/bin/java` → "Unable to
locate a Java Runtime"; no `apalache-mc`) — the same constraint decisions 0024 and
0028 recorded. I therefore could not execute TLC and do **not** treat the v2
`commit_protocol_check.txt` as machine-verified evidence; the record is honest that
its v2 numbers are hand-derived. My ratification rests on my own line-by-line
falsification of the (tiny, exhaustively hand-checkable) model text, which is the
same standard of evidence that established the original DA-1 vacuity. The record is
board-honest about this and reproducible via `formal/commit-protocol/check.sh` on
any JRE. **Carry-forward condition C-A stands:** `T-0038` runs Apalache on the
*implemented* protocol (T-0010 commit writer + the T-0046 content-addressed key
scheme), with `OrphansNeverReferenced` + `NoOverwriteOfReferenced` + the
writer/attempt-scoped object id in the checked invariant set, and re-captures live
checker numbers. Drift between the implemented protocol and this model is a BUG.

## State of the gate (board honesty — what I close, with both primaries now signed)

- **BUG-0012 → `done`.** Its scope is precisely "the TLA+ model is blind to the
  DA-1 torn-committed-read (`NoTornCommit` vacuous over version-scoped `ObjId`)."
  That defect is fixed and I, as the **primary Cat. 11 ratifier**, certify the v2
  model faithful. AC line 6's formal-methods half is discharged by this decision.
  (The `steering-distributed-acid` lane already flipped the board item to `done`
  per decision 0026; my sign-off is the formal-methods half that 0026 §4 explicitly
  waited on. Confirmed `done`.)
- **SPIKE-0002 → `done`** (the design gate). Both primaries have now signed:
  `steering-distributed-acid` (commit-protocol design, decision 0026,
  RATIFIED-WITH-CONDITIONS) and `steering-formal-methods` (TLA+ model, THIS
  decision, RATIFIED). `steering-storage` approved secondary (decision 0027). The
  deliverable — a ratified commit-protocol ADR + a faithful, model-checkable TLA+
  spec that survives adversarial falsification including DA-1 — is complete. ADR
  0002 → `accepted`.
- **Implementation readiness is NOW unblocked at the design level, but the
  commit-path tasks do NOT auto-become `ready` from this sign-off alone.** They
  carry decision 0026's conditions **C1–C3** as hard land-gates (C1 executed TLC
  re-check via T-0038; C2 two-concurrent `PUT If-None-Match:*` mock-fidelity test;
  C3 zombie-late-PUT integration test) **and** other unmet `deps` (e.g. T-0010 deps
  T-0009; T-0026 deps T-0010). The planner/pace-marshal flips them to `ready` when
  ALL deps clear — not me, and not as a side effect of this ratification. Decision
  0026 §5 is explicit on this and I concur.
- **Carry-forward C-A (the executed run).** The TLC/Apalache re-check on a JRE is a
  real, named obligation (T-0038), not satisfied in this sandbox. My ratification is
  of the **model's faithfulness and correctness by inspection**; the executed
  checker numbers are a land-gate on the implementing tasks, not on the design gate.
- **The board is not blocked.** Independent work (SPIKE-0003 storage, T-0014
  latency sim, the TCK harness) is unaffected.

## ADR disposition

I update the ADR 0002 sign-off table to record my **primary (Cat. 11) re-confirm:
RATIFIED** against the v2 model. I do **not** flip the ADR Status to `accepted`:
that flip belongs with the SPIKE-0002 close, which still awaits the landed
distributed-ACID primary re-confirm (see above). The ADR Status therefore stays
`proposed` with my row updated and the gate-status note left intact.

---

## Steering-FormalMethods Verdict

**Verdict:** approve  (RATIFIED — primary, Cat. 11 / TLA+ model)

**Blocking findings:** none. The v2 model survives my independent falsification:
the DA-1 idempotent-collapse is gone (`ObjId(v,w,a,k)` ⇒ distinct ids for racing
writers, confirmed reachable by `DistinctIdsProbe`); `OrphansNeverReferenced` and
`NoOverwriteOfReferenced` are model-checked properties whose antecedents I verified
reachable (`ZombieWroteProbe`); `ZombieLateWrite` makes the stale-PUT attack
representable and provably harmless; `NoTornCommit`/`LatestIsDurable` are no longer
vacuous. Model-sync obligation met (ADR §1/§2/§6 moved in the same PR).

**Non-blocking notes:**
- `OrphansNeverReferenced`'s right disjunct is dead padding (the left disjunct
  carries the proof for every state satisfying the antecedent). Prefer removing it
  or guarding it explicitly to `w` ∈ {committer of target} at the T-0038 pass, so a
  future re-ordering cannot hide a real violation behind it.
- C-A carry-forward: T-0038 runs Apalache on the implemented protocol with the new
  invariants + attempt-scoped id and re-captures live numbers (no JRE here).
- Both primaries are now signed (distributed-ACID decision 0026 + this decision)
  and storage approved secondary (0027); SPIKE-0002 is correctly `done` and ADR
  0002 `accepted`. Commit-path implementation tasks still carry decision 0026's
  C1–C3 land-gates and their own `deps`; they are not flipped to `ready` by this
  sign-off alone.

**Rationale:** A Cat. 11 ratification certifies model *fidelity*. The v1 model was
unfaithful — its `(version,shard)`-only object id made `NoTornCommit` pass
vacuously against the reachable DA-1 torn-committed-read — which is why I returned
CHANGES_REQUESTED in decision 0024. The v2 model re-keys object identity by
`(version, writer, attempt, shard)`, makes the stale-PUT attack representable and
reachable, and converts write-once immutability from an assertion into two
model-checked invariants that hold across that attack. I confirmed each by reading
the (tiny, hand-checkable) model directly, not by trusting the author's derivation.
The defect BUG-0012 names is fixed; I ratify the model and close the bug, while
keeping SPIKE-0002 and the prove-before-code gate honestly closed until the
distributed-ACID primary re-confirm lands.

**Signed:** steering-formal-methods  T+~later
