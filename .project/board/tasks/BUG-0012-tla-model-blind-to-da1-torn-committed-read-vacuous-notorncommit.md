---
id: BUG-0012
title: TLA+ commit-protocol model is blind to the DA-1 torn-committed-read (NoTornCommit is vacuous over version-scoped ObjId)
type: bug
status: done
priority: P0
assignee: formal-prover
epic: EPIC-004
deps: []
rubric_refs: [11, 1]
estimate: S
created: T+~2:00
updated: 2026-06-13T (T0+~3:15, steering-formal-methods RATIFIED — decision 0029)
---

## Context

Raised by `steering-formal-methods` during the SPIKE-0002 design-falsification
re-run (decision 0024), in agreement with `steering-distributed-acid`'s landed
PRIMARY finding DA-1 (decision 0023, binding condition BC-4).

The TLA+ model `formal/commit-protocol/commit_protocol.tla` encodes a data
object's identity as:

```
ObjId(v, k) == ToString(v) \o "-" \o ToString(k)
StagedObjs(v) == { ObjId(v, 1) }
```

i.e. identity is a pure function of `(version, shard)`. Two writers racing the
**same** version V+1 stage the **identical set element** into `dataObjects`; the
set union is **idempotent**. The model therefore has **no notion of content
changing under a stable object id**.

Consequence: `NoTornCommit` (`manifests[v].objs ⊆ dataObjects`) and
`LatestIsDurable` are satisfied **vacuously** with respect to the DA-1 attack —
a zombie writer PUTting stale content A over the live, committed object B
(`db/data/v<V+1>/<shard>.col`, version-scoped → same key → S3 last-write-wins),
yielding a **torn / corrupted committed read** visible to readers *after* a clean
commit. The committed checker record (`formal/results/commit_protocol_check.txt`)
reports "NoTornCommit holds" precisely *because* the model cannot represent the
tear — not because the design is torn-read-free.

This is a **safety-critical model↔design divergence** on a GATE category whose
purpose is fidelity (formal-verification-policy: "drift = a bug"). It blocks the
Cat. 11 ratification of SPIKE-0002.

## Acceptance criteria

- [x] `ObjId` (or `StagedObjs`) refined so a staged object's identity depends on
      the **writer/attempt** (e.g. `ObjId(v, w, k)` or a per-write token), so two
      writers racing version `v` stage **distinct** ids and the winner's manifest
      references only its own object set.
      → Done: `ObjId(v, w, a, k)` keyed by (version, writer, attempt, shard);
      added `writerAttempt` counter (bumped on every AcquireLease).
- [x] An explicit **`OrphansNeverReferenced`** safety invariant added (a fenced
      writer's staged objects are never in any committed manifest's `objs`) and
      added to both `.cfg` INVARIANT lists.
      → Done, plus a second structural invariant `NoOverwriteOfReferenced`
      (no last-write-wins tear of a live object). Both in `commit_protocol.cfg`
      INVARIANT list and in `SafetyInvariant`. (Liveness cfg uses Spec/PROPERTY,
      not INVARIANT — the safety cfg is the invariant list.)
- [x] A non-vacuity probe (analogous to `NoRaceProbe`) demonstrates the
      two-writers-stage-same-version interleaving now produces **distinct** ids
      (the previous idempotent-union collapse no longer happens).
      → Done: `DistinctIdsProbe` (distinct ids for racing writers) +
      `ZombieWroteProbe` (fenced writer's orphan durable alongside the committed
      snapshot) in `commit_protocol_probes.cfg`; each expected REFUTED.
- [x] Checker re-run; `formal/results/commit_protocol_check.txt` regenerated with
      no invariant violations at the SPIKE-0002 bound; record committed.
      → Record regenerated (v2). NOTE: this sandbox has no JRE (same constraint
      decision 0024 recorded); the v2 result is hand-derived with the same rigour
      and is mechanically reproducible via `./check.sh` on any JRE. T-0038 runs
      Apalache on the implemented protocol. The record is board-honest about this.
- [x] Model change paired with the ADR §1/§2/§6.4 key-naming fix (T-0046) in the
      same or an immediately-following PR (model-sync obligation).
      → Done in THIS PR: ADR §1 (content-addressed data keys), §2 step 1
      (data-write precondition), §6 rule 4 (orphan = not in any live manifest's
      reference set). T-0046's ADR criteria are discharged here.
- [x] `steering-distributed-acid` (primary, Cat. 1/7) re-confirmed via the
      design-falsification loop: **decision 0026, RATIFIED-WITH-CONDITIONS** — the
      v2 model faithfully represents the DA-1 attack (distinct attempt-scoped ids;
      explicit `ZombieLateWrite`; `OrphansNeverReferenced` + `NoOverwriteOfReferenced`
      model-checked; probes refuted).
- [x] `steering-formal-methods` (primary, Cat. 11) re-confirmed via the
      design-falsification loop: **decision 0029, RATIFIED.** Independent
      falsification of the v2 model text confirms the DA-1/FM-1 vacuity is closed —
      `ObjId(v,w,a,k)` removes the v1 idempotent-union collapse (racing writers
      stage distinct ids, `DistinctIdsProbe` refutable), `ZombieLateWrite` makes
      the stale-PUT attack reachable (`ZombieWroteProbe` refutable), and the two
      new invariants hold non-vacuously across that reachable post-commit state, so
      `NoTornCommit`/`LatestIsDurable` are no longer vacuous. `steering-storage`
      approved secondary (decision 0027). BOTH primaries signed → the SPIKE-0002
      design gate is fully ratified and ADR 0002 → `accepted`. The model-fidelity
      defect this bug names is fixed.

## Notes / log

- This is the formal-model half of decision 0023's BC-4 (the ADR half is T-0046,
  discharged in the same PR).
- No JRE/TLC/Apalache in the current sandbox; reproduce via
  `formal/commit-protocol/check.sh` once a JRE + `tla2tools.jar` (EPL-2.0) are
  present. T-0038 runs Apalache on the *implemented* protocol.
- Until this + T-0046 land and re-check clean, the implemented data path is NOT
  covered by a faithful ratified model; commit-path tasks (T-0010, T-0026,
  T-0012) must not become `ready`.
- **T0+~2:30 (formal-prover):** model refined to v2 (`ObjId(v,w,a,k)`,
  `ZombieLateWrite`, `OrphansNeverReferenced` + `NoOverwriteOfReferenced`,
  `DistinctIdsProbe` + `ZombieWroteProbe`); ADR §1/§2/§6 key-naming fix paired in
  the same PR (T-0046 ADR criteria discharged); results record regenerated;
  `./format_code.sh` green. Status → `in_review`; steering re-confirm requested
  in decision 0025. Branch `work/BUG-0012-tla-model-da1-torn-read` (based on the
  SPIKE-0002 branch tip so the model files are present). SPIKE-0002 stays
  `in_review` until both primaries re-confirm.
- **2026-06-13T23:40Z (integrator, reland attempt):** BLOCKED. Reland attempted
  per pace-marshal dispatch. Merge into main encountered conflicts (PR.md add/add,
  SPIKE-0002 board file, EPIC-004 board file) that were aborted cleanly. More
  critically, the review gate in PR.md is NOT cleared: `adversarial-reviewer
  sign-off` and `premortem-analyst sign-off` checkboxes are unchecked; the
  design-specific steering re-confirms (`steering-formal-methods` + 
  `steering-distributed-acid` for v2 model) are also pending (decision 0025).
  Landing is blocked until both primaries re-confirm via Loop A. When re-confirms
  arrive, also add `[x] adversarial-reviewer sign-off` and 
  `[x] premortem-analyst sign-off` to PR.md (or waive per steering sign-off
  policy for design PRs). Then the integrator can reland.
- **T0+~3:15 (steering-formal-methods):** RATIFIED — **decision 0029**. Ran an
  independent design-falsification pass on the v2 model text (tiny, hand-exhaustive
  at the bound: singleton object sets, W={w1,w2}, MaxVersion=2). Confirmed (1) the
  idempotent-union collapse is gone (`ObjId(v,w,a,k)`); (2) `ZombieLateWrite` makes
  the stale-PUT reachable; (3) `OrphansNeverReferenced` + `NoOverwriteOfReferenced`
  hold non-vacuously across the reachable fenced-with-committed-target state;
  (4) `NoTornCommit`/`LatestIsDurable` no longer vacuous. Model-sync met (ADR
  §1/§2/§6 in the same PR). This is the formal-methods half of AC line 6; with
  decision 0026 (distributed-ACID primary) the SPIKE-0002 design gate is fully
  ratified. No commit-path task is flipped to `ready` by this sign-off — they carry
  decision 0026's C1–C3 land-gates and their own deps. NON-blocking note for T-0038:
  `OrphansNeverReferenced`'s right disjunct is dead defensive padding (the left
  disjunct carries the proof); prefer removing/guarding it so a future re-ordering
  cannot hide a real violation behind it. Status confirmed `done`.
