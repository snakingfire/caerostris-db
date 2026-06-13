# Decision 0013 — SPIKE-0002 commit-protocol ADR + TLA+ model: steering ratification request

- **Date / marker:** 2026-06-13T19:25:00Z (≈ T0+1:01)
- **Author / role:** `formal-prover`
- **Type:** design-falsification-loop entry + steering ratification **request**
  (not yet ratified — this record opens the loop)
- **Status:** `in_review` — awaiting `steering-distributed-acid` +
  `steering-formal-methods` sign-off
- **Rubric:** Cat. 1 (ACID, GATE, w14), Cat. 7 (concurrency/attach, GATE, w8),
  Cat. 11 (formal, GATE, w6)
- **Artifacts under review:**
  - `docs/adr/0002-s3-commit-protocol.md` (the protocol ADR)
  - `formal/commit-protocol/commit_protocol.tla` (the TLA+ model)
  - `formal/commit-protocol/commit_protocol.cfg` (bounded-check config)
  - `formal/results/commit_protocol_check.txt` (checker run record)
- **Related:** SPIKE-0002 (this), SPIKE-0005 (pre-ratification constraints —
  research complete), decisions 0001 (storage F1–F3), 0004 (distributed-ACID
  findings #2/#3/#4), EPIC-001, EPIC-004, EPIC-006, T-0009/T-0010/T-0011/T-0012,
  T-0038 (Apalache check on implemented protocol).

## What is being submitted for ratification

An **immutable-versioned-manifest, create-only-CAS commit protocol** for
S3-native single-writer / multi-reader ACID. Commit = the atomic creation of a
new immutably-named manifest object via `PUT If-None-Match:*`; fencing is a
store-enforced property of that create-only PUT, **not** of lease belief.

The companion TLA+ model encodes the protocol and **four safety invariants** plus
a liveness property, checked at a bounded scope (2 writers, 2 readers, versions
0..2, lease epochs 0..3 — the SPIKE-0002 suggested bound, which includes the
zombie-writer takeover interleaving).

## How each pre-registered falsification scenario is discharged

The steering committee pre-registered (decisions 0001/0004) the scenarios this
design must survive before ratification. Mapping:

| Pre-registered finding | Where addressed | TLA+ encoding |
|------------------------|-----------------|---------------|
| 0004 #2 / SPIKE-0005 C1 — **name the exact CAS primitive** | ADR §3: `PUT If-None-Match:*` (create-only), explicitly **not** `If-Match` body-CAS; mock-fidelity test mandated | `SwapManifestOk` guarded by `~IsCommitted(target)` (create-only); abstracts S3 to create-only semantics only |
| 0004 #3 / SPIKE-0005 C2 — **fencing must not rest on lease belief; restate `writer_count<=1`** | ADR §4: zombie-writer refuted by create-only CAS; lease is an optimisation only | invariant **`AtMostOneCommitPerVersion`** (replaces `writer_count<=1`); `ExpireLease` + woken `wrote` writer fenced by `SwapManifestFenced` |
| 0004 #4 / SPIKE-0005 C3 — **durability ordering barrier** | ADR §2 step 1, §7: all data durable before swap; commit-ack==swap-ack; orphans GC-able | `SwapManifestOk` guard `writerObjs[w] \subseteq dataObjects`; invariants `NoTornCommit`, `LatestIsDurable` |
| 0001 F3 — **GC safety, no central pin registry** | ADR §6: TTL'd pins + grace window; GC only under lease; master-less ⇒ no GC | `GCOldVersion` gated on `v \notin PinnedVersions` ∧ `v # latest`; invariant **`GCSafety`** |
| 0004 non-blocking — **SI level claimed** | ADR §8: claim SI (floor); serializable for read-only txns | `SnapshotIsolation` invariant (pinned reader sees only V's immutable object set) |
| 0004 non-blocking — **2nd writer reject, not queue** | ADR §4: reject + optional client backoff; no server-side queue | lease acquisition gated on free/expired only |

## Model-check status (Cat. 11 evidence)

- **Tooling:** Apalache (preferred) / TLC (fallback) — both EPL-licensed,
  open-source (open-source-guardrails compliant). Apalache is **not yet in the
  Nix shell**; T-0038 adds it and runs the check against the *implemented*
  protocol.
- **This submission — CHECK RUN AND PASSED (TLC):** the bounded model was
  exhaustively model-checked. **Safety: 7406 distinct states, NO invariant
  violations** (all six invariants + TypeOK). **Liveness:
  `WriterEventuallyCommits` holds** under weak fairness. A **non-vacuity probe
  confirms the zombie-writer race is reachable**, so the safety result is
  meaningful. Full output committed to
  `formal/commit-protocol/../results/commit_protocol_check.txt`; reproduce with
  `formal/commit-protocol/check.sh`.
- **Sync obligation:** any change to the implemented commit phase sequence
  (EPIC-004 / T-0010) updates the `.tla` in the same/next PR and re-runs the
  checker. Drift = a BUG (per `formal-verification-policy.md`).

## Requested action (steering)

1. `steering-distributed-acid`: confirm the protocol refutes the zombie-writer
   split-brain (ADR §4) and that the SI/serializable claim (ADR §8) matches what
   the model proves; verify the durability barrier and failure-mode table (§7).
2. `steering-formal-methods`: confirm the four invariants
   (`NoTornCommit`, `SnapshotIsolation`, `AtMostOneCommitPerVersion`, `GCSafety`,
   + `LatestIsDurable`) faithfully encode atomicity + isolation + fencing, and
   that the bounded scope is adequate (or specify a larger bound for T-0038).
3. Record the verdict (ratified / ratified-with-conditions / changes_requested)
   in the ADR §Sign-off and append to this decision. The SPIKE-0002 board item
   stays `in_review`, and EPIC-001/EPIC-004 commit-path tasks stay `backlog`,
   until both sign off.

## Why this does not block the board now

Per the operating model, the design is committed and the loop is open; dependent
implementation tasks were already `backlog`-gated on SPIKE-0002 by the planner
(design-before-code), so nothing that was previously `ready` is blocked by this
record. Independent work (storage format SPIKE-0003, latency sim, TCK harness)
proceeds in parallel.

## Steering verdicts (appended as they arrive)

Routing (`steering-committee.md` §Sign-off): **primary** `steering-distributed-acid`
(commit/isolation/fencing/attach), **secondary** `steering-formal-methods` (TLA+).
`steering-storage` adds a **secondary storage-domain** sign-off because it owns
decision 0001 F2 (CAS primitive) + F3 (GC safety), both discharged here. The
**primary** sign-off is still required for the gate to open.

- [ ] `steering-distributed-acid`: **PENDING** — primary sign-off required. Until
      this is recorded, SPIKE-0002 stays `in_review` and EPIC-001/EPIC-004
      commit-path tasks stay `backlog`. (Verify: zombie-writer refutation §4,
      SI/serializable claim §8, durability barrier + failure-mode table §7.)
- [x] `steering-formal-methods`: **APPROVE** — 2026-06-13 (≈T0+1:10), decision
      **0014**. Two non-blocking tracked conditions (C-A Apalache + `OrphansNeverReferenced`
      on T-0038; C-B mock-fidelity test green before commit-path tasks ready).
- [x] `steering-storage`: **APPROVE** (secondary, storage domain) — 2026-06-13,
      decision **0015**. F2 + F3 discharged. One binding constraint carried to
      **SPIKE-0003** (cross-version object sharing ⇒ ref-counted GC; does not gate
      this ADR). Two non-blocking notes (grace-window constant test; Cat. 2 ≥90
      still needs SPIKE-0003's latency-serving layout / F1). Verdict block in PR.md
      and ADR §Sign-off.
