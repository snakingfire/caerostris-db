# formal/commit-protocol — TLA+ model of the S3 commit protocol

Machine-checked model of the caerostris-db single-writer / multi-reader commit
protocol on object storage. Evidence for rubric **Cat. 11** (formal verification)
and **Cat. 1** ("behaviour matches the TLA+ model"). Design: ADR
[`docs/adr/0002-s3-commit-protocol.md`](../../docs/adr/0002-s3-commit-protocol.md).
Ratification request: `.project/decisions/0013-commit-protocol-steering-ratification-request.md`.

## Files

| File | Purpose |
|------|---------|
| `commit_protocol.tla` | The spec: state, actions, invariants, liveness. |
| `commit_protocol.cfg` | Bounded **safety** config (reader-symmetry). |
| `commit_protocol_liveness.cfg` | **Liveness** config (fair `Spec`, no symmetry). |
| `commit_protocol_probes.cfg` | **Non-vacuity probes** (each expected REFUTED). |
| `../results/commit_protocol_check.txt` | Committed checker output (the evidence). |

## Invariants (steering-mandated; decisions 0001 / 0004 / 0023 / 0024)

- `NoTornCommit` — no reader resolves a manifest whose data objects are not all durable.
- `SnapshotIsolation` — a reader pinned at V reads only V's immutable object set.
- `AtMostOneCommitPerVersion` — the **corrected** fencing invariant (replaces the
  naive `writer_count <= 1`): at most one commit succeeds per manifest version,
  via create-only conditional PUT — refutes the zombie-writer split-brain.
- `GCSafety` — GC never deletes an object a live pinned reader references.
- `OrphansNeverReferenced` — **(DA-1 / BC-4, BUG-0012)** a fenced/crashed writer's
  staged objects are never referenced by any committed manifest. This is what makes
  a zombie writer's late stale PUT harmless: with per-write-attempt-unique object ids
  (modelling the ADR's content-addressed key), its PUT lands on its own orphan key,
  never a live, referenced one.
- `NoOverwriteOfReferenced` — **(DA-1 / BC-4)** no object id a committed manifest
  references is owned by any other writer/attempt, so a stale PUT can never
  last-write-wins over a live object. Converts "write-once immutability" from an
  assertion into a model-checked property.
- `LatestIsDurable` — the resolved `latest` always names a complete, durable manifest.
- Liveness: `WriterEventuallyCommits` (under weak fairness).

### The DA-1 / BC-4 refinement (BUG-0012, decisions 0023 / 0024)

v1 keyed a data object as `ObjId(v, k)` — a pure function of `(version, shard)`.
Two writers racing the **same** version staged the *identical* set element, so
`dataObjects ∪ writerObjs` was idempotent and the model had **no notion of content
changing under a stable id**. `NoTornCommit` therefore held *vacuously* against the
DA-1 attack (a zombie writer PUTting stale content over a live committed object's key
— S3 last-write-wins). v2 keys an object as `ObjId(v, w, a, k)` by `(version, writer,
attempt, shard)`: racing writers stage **distinct** ids, the new `ZombieLateWrite`
action makes the stale-PUT attack **representable**, and `OrphansNeverReferenced` +
`NoOverwriteOfReferenced` prove it can never corrupt a committed snapshot.

## Reproduce the check

Apalache is the preferred checker (bounded BMC with type annotations). It is not
yet in the Nix shell; T-0038 adds it and runs it against the *implemented*
protocol. Meanwhile TLC (the fallback named in the formal-prover agent def and
`formal-verification-policy.md`) checks this bounded model exhaustively.

```bash
# TLC (fallback). Requires a JRE and tla2tools.jar (EPL-2.0); both open-source.
./check.sh                       # runs SANY + safety + liveness + probes

# Apalache (preferred; once available in the shell):
apalache-mc check \
  --inv=NoTornCommit --inv=SnapshotIsolation \
  --inv=AtMostOneCommitPerVersion --inv=GCSafety \
  --inv=OrphansNeverReferenced --inv=NoOverwriteOfReferenced \
  --inv=LatestIsDurable \
  --length=20 commit_protocol.tla
```

The committed `../results/commit_protocol_check.txt` is the canonical record.

## Bound

`Writers={w1,w2}`, `Readers={r1,r2}`, `MaxVersion=2`, `MaxLeaseEpoch=3` — the
SPIKE-0002 suggested bound (≤2 readers, ≤2 writer epochs; `MaxLeaseEpoch` also
bounds the per-writer attempt counter). Three non-vacuity probes
(`NoRaceProbe`, `DistinctIdsProbe`, `ZombieWroteProbe` in
`commit_protocol_probes.cfg`) prove the same-version race, distinct racing ids,
and the reachable zombie late-write are all *reachable* within this bound, so the
safety pass is meaningful, not trivially satisfied.

## Model-sync obligation

Any change to the implemented commit phase sequence (EPIC-004 / T-0010, and the
data-object key scheme / T-0046) must update `commit_protocol.tla` in the same or
an immediately following PR and re-run the checker. **Drift between model and code
is a bug** — file `BUG-NNNN`. T-0038 re-runs Apalache against the implemented
protocol.
