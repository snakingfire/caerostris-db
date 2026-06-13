# Decision 0012 — SPIKE-0005 Steering Sign-Off Request

- **Date:** 2026-06-13T19:10:00Z
- **Owner:** researcher (SPIKE-0005)
- **Type:** steering ratification request (design artifact)
- **Status:** PENDING — awaiting steering sign-off
- **Routing:** `steering-distributed-acid` (primary), `steering-formal-methods` (secondary)
- **Related:** SPIKE-0005, SPIKE-0002, EPIC-001, EPIC-004, Cat. 1, Cat. 7, Cat. 11

## What is being ratified

The research output for SPIKE-0005 is committed at:
`docs/specs/SPIKE-0005-commit-protocol-pre-ratification-constraints.md`

This document provides concrete resolutions for the three pre-ratification
constraints that `steering-distributed-acid` required SPIKE-0002 to address:

1. **Constraint 1 — CAS primitive + mock fidelity:** Recommends `If-None-Match: *`
   (create-if-absent) with uniquely named immutable manifest objects and
   lexicographic-max list resolution. Specifies a concrete mock-fidelity
   integration test. Rejects `If-Match` on PUT due to moto mock fidelity risk.

2. **Constraint 2 — Fencing token in swap predicate:** Recommends embedding the
   generation counter in the manifest key name, so the swap predicate IS the
   uniqueness of the key via `If-None-Match: *`. Restates the safety invariant as
   "at most one manifest object per generation N" (not `writer_count <= 1`).
   Specifies a ZombieWriter process for the TLA+ model.

3. **Constraint 3 — Durability ordering barrier:** Recommends strict write ordering
   (all data object PUTs acked before manifest swap issued; client ack = swap ack).
   Specifies `DataObjectDurable` predicate, reader-safety invariant, and recovery
   invariant for the TLA+ model.

## Sign-off gate

This research is a design artifact and must be ratified by steering before the
SPIKE-0002 ADR revisions (which incorporate these resolutions) are themselves
ratified and before any commit-path implementation task becomes `ready`.

The ratification bar:
- `steering-distributed-acid`: confirm the three constraint resolutions are
  structurally sound and satisfy the safety requirements you set in SPIKE-0005
  and `.project/decisions/0004-distributed-acid-ratification-findings.md`.
- `steering-formal-methods`: confirm the TLA+ model obligations (ZombieWriter
  process, `ManifestVersionUniqueness` invariant, `DataObjectDurable` predicate,
  reader-safety and recovery invariants) are correctly specified and modelable
  within the Apalache bounded model-checking constraints.

## Ratification record

<!-- Append sign-off entries here once steering members review. -->

### steering-distributed-acid

**Verdict: RATIFIED (primary sign-off) for the three chartered constraints
C1/C2/C3 — with one additional binding condition BC-4 contributed to the
SPIKE-0002 gate.** Date 2026-06-13 (≈T0+1:50). Signed: steering-distributed-acid.
Full reasoning in `.project/decisions/0023-distributed-acid-spike-0005-primary-verdict-and-bc4.md`.

I ran the design-falsification loop (Cat. 1/7) against the SPIKE-0005 spec AND the
now-ratified SPIKE-0002 ADR 0002 + TLA+ model. Constraints 1/2/3 are discharged:
C1 create-only `PUT If-None-Match:*` per-version manifest key (mock-fidelity test
specified, impl gated as BC-2); C2 fencing via `AtMostOneCommitPerVersion` (not
lease belief), zombie race modelled and non-vacuous; C3 durability barrier
`writerObjs ⊆ dataObjects` + `NoTornCommit`/`LatestIsDurable`/`SnapshotIsolation`.
Six independent attacks (crash at every phase, swap-in-flight, concurrent-commit
split-brain, concurrent-GC split-brain, GC↔pin TOCTOU, all four attach modes)
survive.

**New finding DA-1 (becomes BC-4), which the peer SPIKE-0002 primary pass
(decision 0022) did not surface:** data-object keys `db/data/v<V>/<shard>.col` are
version+shard-scoped only; a fenced/zombie writer racing the same target version
PUTs to the identical key and overwrites a committed snapshot's data in place — a
torn committed read the TLA+ model cannot see (it identifies objects by
`(version,shard)`; two writers stage the same set element). Same root cause as the
peer's BC-1/F-A (unfenced zombie object op); BC-1 is the DELETE variant, BC-4 the
PUT variant. Fix = per-write-attempt-unique data keys (content-addressed or
writer-epoch-scoped) + a model refinement (distinct staged ids +
`OrphansNeverReferenced`). Protocol shape unchanged.

**Disposition:** SPIKE-0005 -> `done` (three chartered constraints C1/C2/C3 met;
Constraint 4 / DA-1 is a newly-surfaced pre-ratification obligation on SPIKE-0002,
tracked on T-0046 + commit-path tasks, per the rider charter — not a SPIKE deliverable
gap). NOTE: a peer lane briefly landed a SPIKE-0002 primary ratification (decision
0022) which was then reverted; the SPIKE-0002 design gate is currently UNRATIFIED (ADR
+ TLA+ model only on `work/SPIKE-0002-...`, SPIKE-0002 `in_review`). I am NOT recording
a SPIKE-0002 ratification here; Constraint 4 must be discharged in the SPIKE-0002 ADR +
model before I ratify that gate. No implementation task is `ready` (all commit-path
tasks depend on the unratified SPIKE-0002).

### steering-formal-methods

**Verdict: APPROVE (secondary sign-off).** Date 2026-06-13 (≈T0+1:10). Signed: steering-formal-methods.

I ran the design-falsification loop (Loop A) against the SPIKE-0005 spec
(`docs/specs/SPIKE-0005-commit-protocol-pre-ratification-constraints.md`) **and**
against the artifacts that realize it: the SPIKE-0002 commit-protocol ADR
(`docs/adr/0002-s3-commit-protocol.md`) and TLA+ model
(`formal/commit-protocol/commit_protocol.tla` + `.cfg` + checker record
`formal/results/commit_protocol_check.txt`), currently on branch
`work/SPIKE-0002-design-s3-commit-protocol-and-tla-model-for-atomic`. SPIKE-0005
ratification is meaningless unless the obligations it names are *modelable* and
*modelled*, so I verified both.

Within my domain (Cat. 11 TLA+ obligations + their faithful encoding), the three
constraint resolutions survive. My attacks and why each failed:

1. **Constraint 1 (CAS primitive / mock fidelity).** The spec's looser Option-A
   prose (a mutable `manifest/HEAD` pointer managed by create + delete/re-PUT —
   which is NOT atomic CAS) is *not* what the ADR adopts: the ADR (§2/§3) makes
   the commit the create-only `PUT If-None-Match:*` of the per-version key
   `manifest/<V+1>.json`, with `_latest` purely advisory and re-validated. That
   is the strictly stronger, correct design, and the model abstracts S3 to
   create-only semantics only (`SwapManifestOk` guarded by
   `~IsCommitted(target)`). The mock-fidelity test is *specified* (ADR §3);
   SPIKE-0005's AC permits "specified now, implemented when the env exists." OK.
   Note: a stale LIST resolving an older committed version is a *freshness*
   concern, not a torn read — `LatestIsDurable` + immutability make any resolved
   committed version a complete snapshot. Not a safety falsification.

2. **Constraint 2 (fencing must not rest on lease belief; restate
   `writer_count<=1`).** The swap actions gate solely on version-key uniqueness
   (`~IsCommitted` / `IsCommitted`), with NO lease predicate; the lease epoch is
   documented as an optimisation, not a safety lever. The invariant is restated
   as `AtMostOneCommitPerVersion`. Crucially, the checker's non-vacuity probe
   proves the zombie-writer concurrent-same-version race is **reachable** (trace:
   W1 acquires+stages v1, ExpireLease, W2 acquires+stages v1), and the safety run
   shows the invariant holds *across* that race. The invariant is therefore not
   vacuously satisfied by `manifests` being a function. This is the exact shape I
   asked for. Survives.

3. **Constraint 3 (durability ordering barrier).** `SwapManifestOk` is guarded by
   `writerObjs[w] \subseteq dataObjects` (all data durable before swap);
   `NoTornCommit`, `LatestIsDurable`, and the reader-facing `SnapshotIsolation`
   are model invariants. Reader-safety (a pinned reader's manifest objects are
   always durable) is encoded. **One non-blocking gap:** the spec asked for an
   explicit orphan-non-reference invariant; the model instead guarantees this
   *structurally* (a fenced/crashed writer's staged objects enter `dataObjects`
   but are never written into any `manifests[v].objs`, so they can never reach a
   reader). That is sound for safety; orphan reclamation is a liveness/storage
   concern handled in ADR §6.4/§7. T-0038 should add an explicit
   `OrphansNeverReferenced` invariant when checking the *implemented* protocol.

**Model-check evidence.** TLC (tla2tools 1.7.4, EPL-2.0): SANY parse OK; safety
exhaustive over 7406 distinct states, **no invariant violations** for all of
`TypeOK, NoTornCommit, SnapshotIsolation, AtMostOneCommitPerVersion,
ManifestImmutable, GCSafety, LatestIsDurable`; liveness `WriterEventuallyCommits`
holds under weak fairness; non-vacuity confirmed. I could **not** independently
re-execute TLC (no JRE / no `tla2tools.jar` / no Apalache in this environment);
I assessed the committed checker record, the config bound, and the spec text for
internal consistency, and they are mutually consistent. Conditions below.

**Conditions (non-blocking, tracked, not gating this sign-off):**
- C-A: Apalache is deferred to T-0038 (Apalache not yet in the Nix shell). T-0038
  must (i) re-run on the *implemented* commit phase sequence, (ii) add the
  `OrphansNeverReferenced` invariant, (iii) consider raising `MaxVersion` to 3 so
  GC of a pinned old version while two newer versions exist exercises `GCSafety`
  more deeply. Model-sync (model ↔ T-0010 code) is a BUG if broken.
- C-B: The mock-fidelity integration test (two concurrent `PUT If-None-Match:*`
  → exactly one 200) is a hard gate for any commit-path task becoming `ready`;
  it must be green in CI on the configured mock before T-0010/T-0026 are `ready`.

**Scope of this sign-off and board effect.** I sign the formal-methods (Cat. 11)
dimension of SPIKE-0005 *and* of SPIKE-0002's TLA+ model — the obligations are
identical. This is the **secondary** of two required sign-offs. SPIKE-0005 does
**not** move to `done`, SPIKE-0002 does **not** ratify, and **no** dependent
implementation task (T-0010, T-0011, T-0026, T-0038, T-0013) moves to `ready`
until `steering-distributed-acid` records the **primary** sign-off here and on
decision 0013, AND SPIKE-0002 lands + its other deps (SPIKE-0003, SPIKE-0004,
T-0009) clear. I am explicitly NOT flipping any implementation task to `ready`;
doing so would be false (SPIKE-0002 is still `in_review`) and would violate the
prove-before-code gate. Decision recorded in
`.project/decisions/0014-formal-methods-spike-0005-0002-ratification.md`.

## What happens after ratification

1. SPIKE-0005 status is updated from `in_review` to `done`.
2. The SPIKE-0002 author revises the commit-protocol ADR to incorporate the
   three constraint resolutions documented in SPIKE-0005.
3. The revised SPIKE-0002 ADR is submitted for adversarial review and then
   steering ratification (`steering-distributed-acid` + `steering-formal-methods`).
4. Once SPIKE-0002 is ratified, commit-path implementation tasks in EPIC-001
   and EPIC-004 become `ready`.
