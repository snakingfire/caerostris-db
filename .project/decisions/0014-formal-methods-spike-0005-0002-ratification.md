# Decision 0014 — Formal-methods ratification of SPIKE-0005 (and the SPIKE-0002 TLA+ model)

- **Date / marker:** 2026-06-13 (≈ T0+1:10)
- **Owner / role:** `steering-formal-methods`
- **Type:** steering ratification (design-falsification Loop A) — **secondary**
  sign-off for the commit-protocol design gate
- **Verdict:** **APPROVE** (with two non-blocking, tracked conditions)
- **Rubric:** Cat. 11 (formal verification, GATE, w6); supports Cat. 1 (ACID,
  GATE, w14) "behaviour matches the TLA+ model"
- **Artifacts reviewed:**
  - `docs/specs/SPIKE-0005-commit-protocol-pre-ratification-constraints.md` (the
    constraints-rider research, on `main`)
  - `docs/adr/0002-s3-commit-protocol.md` (the protocol ADR — on branch
    `work/SPIKE-0002-design-s3-commit-protocol-and-tla-model-for-atomic`)
  - `formal/commit-protocol/commit_protocol.tla` + `.cfg` +
    `commit_protocol_liveness.cfg` + `check.sh` (same branch)
  - `formal/results/commit_protocol_check.txt` (committed checker record)
- **Related:** SPIKE-0005, SPIKE-0002, decisions 0004 (#2/#3/#4), 0012
  (SPIKE-0005 sign-off request), 0013 (SPIKE-0002 ratification request, on the
  SPIKE-0002 branch), EPIC-001, EPIC-004, EPIC-006, T-0009/T-0010/T-0011/T-0012/
  T-0013/T-0026/T-0038.

## What I was asked to do

Run the design-falsification loop on board item **SPIKE-0005** — try to refute
the committed design against the latency theorem, ACID, and interface contracts.
SPIKE-0005 is a *constraints rider* on SPIKE-0002; its three constraints are
discharged inside the SPIKE-0002 ADR + TLA+ model. Ratifying SPIKE-0005 in
isolation would be hollow, so I falsified the obligations **and** their
realization in the model. My authority is Cat. 11 (the TLA+ obligations) and the
formal-methods half of Cat. 1; `steering-distributed-acid` is the primary signer
for the ACID/safety semantics.

## Falsification attempts and outcomes (all survived)

| # | Attack | Outcome |
|---|--------|---------|
| 1 | Constraint 1 — is the named CAS primitive realizable, and does the model match it? | The ADR adopts create-only `PUT If-None-Match:*` of a per-version manifest key (`manifest/<V+1>.json`), `_latest` advisory-only — stronger than the spec's looser Option-A prose (mutable HEAD via create+delete, which is not CAS). The model abstracts S3 to create-only only (`SwapManifestOk` guard `~IsCommitted(target)`). Faithful. Mock-fidelity test specified (ADR §3). **Survives.** |
| 2 | Stale LIST resolves an old version → torn read? | No. A resolved committed version is a complete SI snapshot (`LatestIsDurable` + immutability). Freshness, not safety. **Survives.** |
| 3 | Constraint 2 — zombie writer; is `AtMostOneCommitPerVersion` vacuous? | Swap gated solely on version-key uniqueness, never on lease belief; lease epoch is optimisation only. Non-vacuity probe proves the concurrent-same-version race is *reachable*; invariant holds across it. Not vacuous. **Survives.** |
| 4 | Constraint 3 — durability barrier / reader sees 404 | `SwapManifestOk` guard `writerObjs ⊆ dataObjects`; `NoTornCommit`, `LatestIsDurable`, `SnapshotIsolation` (reader-safety) are model invariants. **Survives.** |
| 5 | All four attach modes + every crash point covered? | ADR §4 maps modes 1–4; `CrashWriter` enabled in any non-idle/non-crashed state; failure-mode table §7 enumerates each crash point. **Survives.** |
| 6 | Snapshot isolation encoded as an invariant (not a comment)? Lease + split-brain encoded? | `SnapshotIsolation` is a TLA+ INVARIANT; `AcquireLease`/`ExpireLease` model the lease; split-brain is flagged by `AtMostOneCommitPerVersion`. **Survives.** |
| 7 | Bound adequacy | Writers={w1,w2}, Readers={r1,r2}, MaxVersion=2, MaxLeaseEpoch=3 — the steering-suggested bound; exhibits the zombie race. Apalache deferred to T-0038. Adequate for this gate. **Survives.** |

## Model-check evidence

TLC (tla2tools 1.7.4, EPL-2.0): SANY parse OK; **safety exhaustive over 7406
distinct states, no invariant violations** (`TypeOK, NoTornCommit,
SnapshotIsolation, AtMostOneCommitPerVersion, ManifestImmutable, GCSafety,
LatestIsDurable`); liveness `WriterEventuallyCommits` holds under weak fairness;
non-vacuity probe confirms the zombie race is reachable. I **could not** re-run
TLC independently (no JRE / `tla2tools.jar` / Apalache in this environment); I
assessed the committed record, the bound config, and the spec text for mutual
consistency and found them consistent. "Looks fine" is not my rationale — the
specific evidence is the committed 7406-state exhaustive pass plus the
reachability probe that makes it non-vacuous.

## Non-blocking conditions (tracked; do not gate this sign-off)

- **C-A (Apalache + invariant additions, owner T-0038):** re-run on the
  *implemented* commit phase sequence; add `OrphansNeverReferenced` (the model
  currently guarantees orphan non-reference structurally, not via a named
  invariant); consider `MaxVersion=3` to deepen `GCSafety` coverage (GC of a
  pinned old version while two newer versions exist). Model↔code drift = a BUG.
- **C-B (mock-fidelity test, owner EPIC-001/EPIC-009):** the two-concurrent
  `PUT If-None-Match:*` → exactly-one-200 test must be green in CI on the
  configured mock **before** any commit-path task (T-0010, T-0026) becomes
  `ready`. If the mock cannot enforce the precondition, the model proves a
  protocol the implementation cannot realize — escalate.

## Board effect (what I am and am NOT doing)

- This is the **secondary** of two required sign-offs (decision 0012 routing:
  `steering-distributed-acid` primary, `steering-formal-methods` secondary).
- **SPIKE-0005 stays `in_review`** until `steering-distributed-acid` records the
  primary sign-off. **SPIKE-0002 stays `in_review`** (decision 0013) for the same
  reason and because its artifacts have not landed on `main`.
- **No implementation task is flipped to `ready`.** T-0010, T-0011, T-0026,
  T-0013, T-0038 all depend on SPIKE-0002 (still `in_review`); T-0010/T-0026 also
  depend on SPIKE-0005; T-0009 also depends on SPIKE-0003 + SPIKE-0004 (both
  `backlog`). Flipping any of them now would be false and would violate the
  prove-before-code gate. The board flip is correctly **gated** and I leave it so.
- I recorded my verdict in decision 0012 (SPIKE-0005 sign-off request) and append
  the SPIKE-0005 board notes. When SPIKE-0002 lands, my identical formal-methods
  sign-off should also be appended to decision 0013 §Steering verdicts.

## Why APPROVE, not changes_requested

The three pre-registered falsification scenarios that gate the commit-protocol
design are correctly resolved and *faithfully encoded* as model-checked
invariants, and the safety result is non-vacuous over the reachable zombie-race
state space. The latency theorem is untouched (commit is a write path; reads are
unaffected; manifest LIST/GET folds into the K-phase budget). No ACID or
interface contract is broken. The two open items are tracked, non-safety, and
correctly deferred to the implementation-time check (T-0038) and the CI
mock-fidelity gate — neither warrants holding the design gate open.
