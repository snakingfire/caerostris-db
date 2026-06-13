# Decision 0027 — Storage-domain sign-off of SPIKE-0002 (S3 commit protocol + TLA+ model)

> **Renumbered 0015 → 0027 on land** (steering-distributed-acid, SPIKE-0002
> ratification, decision 0026): `0015` was already taken on `main` by
> `0015-formal-methods-spike-0001-ratification.md` (a different decision). This is
> the `steering-storage` secondary sign-off authored on the SPIKE-0002 work branch
> and brought to `main` as part of the SPIKE-0002 ratification land. Content
> unchanged from the branch original; only the file number is corrected.

- **Date / marker:** 2026-06-13 (≈ T0+5:00)
- **Owner / role:** `steering-storage`
- **Type:** steering ratification (design-falsification Loop A) — **secondary
  storage-domain** sign-off for the commit-protocol design gate
- **Verdict:** **APPROVE** (1 binding constraint carried to SPIKE-0003; 2
  non-blocking notes — none gates this ADR)
- **Rubric:** Cat. 2 (storage format & S3 commit, GATE, w12) — storage-bearing
  aspects of the commit protocol; supports Cat. 1 (ACID, GATE, w14) and Cat. 7
  (concurrency / attach modes, GATE, w8) on the storage side.
- **Artifacts reviewed (branch `work/SPIKE-0002-design-s3-commit-protocol-and-tla-model-for-atomic`):**
  - `docs/adr/0002-s3-commit-protocol.md` (the protocol ADR)
  - `formal/commit-protocol/commit_protocol.tla` (+ `.cfg`, liveness cfg, `check.sh`, `README.md`)
  - `formal/results/commit_protocol_check.txt` (TLC checker record)
  - `.project/decisions/0013-...` (ratification request) and `0014-...`
    (formal-methods secondary sign-off)
- **Related:** SPIKE-0002, SPIKE-0003 (storage format — the binding constraint
  below lands here), SPIKE-0005, decisions 0001 (storage F1/F2/F3), 0004 (#2/#4),
  0013, 0014; EPIC-001, EPIC-004, EPIC-006; T-0038.

## Authority / why steering-storage signs at all

Per `steering-committee.md` the **commit protocol / isolation guarantee** is the
**primary** lane of `steering-distributed-acid` (secondary `steering-formal-methods`).
`steering-storage` is the **secondary consultant** here because the commit
protocol's *storage-bearing* surface — manifest layout & versioning, GC of old
versions, and concurrent-reader version-pinning — is Cat. 2, my domain, and
because decision 0001 committed me by name to **refuse storage ratification until
findings F2 (the CAS primitive) and F3 (GC safety with no central pin registry)
are discharged.** Both are discharged in this ADR. This sign-off is **necessary
but not sufficient**: the gate opens only when `steering-distributed-acid`
(primary) records its verdict in decision 0013.

## Falsification attempts and outcomes (storage domain — all survived)

| # | Attack | Outcome |
|---|--------|---------|
| 1 | **F2 — is the swap primitive a real CAS, or hand-waved "conditional PUT or equivalent"?** | ADR §3 names **create-only `PUT If-None-Match:*`** precisely, explicitly rejects `If-Match` body-CAS as non-portable, and the model abstracts S3 to *exactly* create-only (`SwapManifestOk` guard `~IsCommitted(target)`). Mock-fidelity test (two concurrent creates → one 200 / one 412) is mandated as the model→impl bridge. **F2 discharged. Survives.** |
| 2 | **Torn read via stale LIST** — a reader resolving `max(LIST manifest/)` could resolve an older version. | Any committed version is a *complete, immutable* SI snapshot (`LatestIsDurable` + `ManifestImmutable` + `NoTornCommit`). Resolving an older version is a **freshness**, not **safety**, property; single-writer means no write the reader is obliged to see. **Survives.** |
| 3 | **F3 — GC deletes an object a reader is mid-read on.** | GC runs **only under the writer lease**, never on `latest`, never on a version with a live pin (`v ∉ PinnedVersions`), with a grace window strictly > max reader-session lifetime; `GCSafety` is a **checked TLA+ invariant** (no live pin's objects are ever absent). Master-less mode has no writer ⇒ no GC ⇒ no deleter. **F3 discharged. Survives.** |
| 4 | **Concurrent-reader sees a torn commit during V+1.** | A commit only ever *creates new* keys (`v<V+1>/...`, `manifest/<V+1>.json`); the pinned reader's object set is immutable and version-scoped, so it is stable for the whole transaction. `SnapshotIsolation` holds across 7406 exhaustively-checked states, **non-vacuously** across the reachable zombie-writer race. **Survives.** |
| 5 | **Cross-version object sharing breaks wholesale GC.** | `GCOldVersion` deletes `manifests[v].objs` *wholesale*; ADR §6 rule (c) + model `ObjId(v,k)` assume **each version owns its objects exclusively**. Under that assumption the protocol is correct. **But** it is a *latent* falsification under a likely SPIKE-0003 decision (delta encoding / shared unchanged shards to bound write amplification on a 10B-edge graph): then wholesale per-version deletion could reclaim an object a newer live version still references. **Recorded as a binding constraint on SPIKE-0003 (below), NOT a falsification of the commit protocol** (atomicity is independent of object sharing). **Survives for this gate.** |

## Binding constraint (carried to SPIKE-0003; does NOT gate this ADR)

**BC-1 — cross-version object sharing requires reference-counted GC.** If the
storage-format spike (SPIKE-0003) adopts cross-version object sharing or delta
encoding (so a manifest at V+1 references some objects also referenced by V), the
GC model in ADR §6 and the TLA+ `GCOldVersion`/`GCSafety` actions **must** be
upgraded from "a version owns its objects exclusively, delete them wholesale" to a
**reference-count / union-of-live-object-sets** discipline (GC may reclaim an
object only when *no* live-or-pinned manifest references it). `steering-storage`
will enforce this at SPIKE-0003 ratification. The commit protocol itself is
unaffected; this is a forward constraint, recorded so the GC invariant is not
silently invalidated when the layout is pinned.

## Non-blocking notes (tracked; do not gate this sign-off)

- **N-1 (grace-window constant, EPIC-001 T-0012 / EPIC-006):** the *shape* (grace
  > max reader-session lifetime, pin TTL'd) is correctly fixed in the ADR; the
  constants are an implementation tuning. Add a property/integration test for a
  slow reader whose pin renewal lands inside the grace window under modest clock
  skew, asserting its version is never collected.
- **N-2 (Cat. 2 not closed here):** this ADR pins the **commit half** of the
  format only. The latency-serving on-object layout (adjacency chunking, early
  range-GET abort — decision 0001 **F1**) remains SPIKE-0003's burden; Cat. 2 ≥ 90
  is not achievable on this ADR alone.

## Board effect (what I am and am NOT doing)

- This is the **secondary storage-domain** sign-off. Together with
  `steering-formal-methods` (decision 0014, APPROVE) two secondaries are now in.
- **The gate stays CLOSED.** The **primary** `steering-distributed-acid` sign-off
  is still pending (decision 0013). **SPIKE-0002 stays `in_review`.** **No**
  commit-path implementation task (EPIC-001/EPIC-004: T-0010, T-0011, T-0012,
  T-0013, T-0026, T-0038) is flipped to `ready`. Flipping any now would violate
  the design-before-code gate and `steering-committee.md` §Sign-off (primary
  required).
- **Landing note for the integrator:** these artifacts have **not** landed on
  `main` (there is no `formal/` directory on `main`). Landing is the integrator's
  call under Loop B, but should not pre-empt the primary ratification — land the
  branch and open the gate only once decision 0013 records the
  `steering-distributed-acid` verdict.

## Why APPROVE, not changes_requested

The four pre-registered storage-domain falsification scenarios (decision 0001
F2/F3, decision 0004 #2/#4) are correctly resolved and faithfully encoded as
model-checked invariants over a non-vacuous reachable state space that includes
the zombie-writer race. I could not break concurrent-reader safety or GC safety
under the stated exclusive-ownership assumption. The one place the model's
simplification could later bite (cross-version object sharing) is a *forward*
constraint on SPIKE-0003, not a defect in this commit protocol, and is recorded as
such. The latency theorem is untouched: commit is a write path; reads are
unaffected; manifest LIST/GET folds into the K-phase budget (one round-trip at
open, advisory `_latest` hint reduces it to a GET).
