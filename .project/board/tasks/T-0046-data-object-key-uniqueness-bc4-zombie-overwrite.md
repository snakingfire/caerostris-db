---
id: T-0046
title: Discharge Constraint 4 / DA-1 (BC-4) — per-write-attempt-unique data-object keys + TLA+ model refinement
type: task
status: readypriority: P0
assignee:
epic: EPIC-004
deps: [SPIKE-0002]
rubric_refs: [1, 7, 11]
estimate: S
created: 2026-06-13T20:20:00Z
updated: 2026-06-13T20:20:00Z
---

## Context

Filed by `steering-distributed-acid` from the SPIKE-0005 primary verdict
(`.project/decisions/0023-distributed-acid-spike-0005-primary-verdict-and-bc4.md`;
SPIKE-0005 Constraint 4). This is a **pre-ratification obligation on the SPIKE-0002
commit-protocol gate** and a hard pre-`ready`/land-gate for commit-path implementation.

**Finding DA-1 (the bug):** the SPIKE-0002 ADR data-object key is
`db/data/v<V>/<shard>.col` — version+shard-scoped only, "written once" *asserted* but
not *enforced*. An unconditional S3 PUT to an existing key is last-write-wins. Two
writers targeting the same version V+1 write to the **identical** key. A fenced/zombie
writer (W1) that wakes after W2 has committed V+1 can PUT its stale content to the same
key, overwriting W2's **committed, manifest-referenced** data **in place** -> a
torn/corrupted committed read visible to readers, produced *after* a clean commit.

**Why the TLA+ model misses it:** `ObjId(v,k) == ToString(v) o "-" o ToString(k)` makes
a data object's identity a pure function of `(version, shard)`. Two writers staging "the
same" object add the identical set element to `dataObjects`; the union is idempotent, so
`NoTornCommit` (`objs \subseteq dataObjects`) passes vacuously w.r.t. this attack. The
model proves write-once immutability the implementation does not realize — a
safety-critical model<->implementation divergence (formal-verification-policy: "drift =
a bug").

Same root cause as the GC-delete-fencing finding (F-A/BC-1) raised on SPIKE-0002: an
unfenced zombie-writer object operation. BC-1 is the DELETE variant (removes a live
object); this is the PUT variant (overwrites a live object). The fix here is a key-naming
constraint + a model refinement; it does NOT change the manifest-CAS / fencing /
durability-barrier / pinning / GC / attach-mode design.

## Acceptance criteria
- [ ] SPIKE-0002 ADR §1/§2/§6 updated so **every data-object key is unique per write
      attempt**: preferred content-addressed (`db/data/<content-hash>/<shard>.col`,
      enabling create-only `If-None-Match:*` on data PUTs for defence in depth), or
      writer-epoch/attempt-scoped (`db/data/v<V+1>/<epoch-or-uuid>/<shard>.col`). The
      manifest records the exact keys it references.
- [ ] ADR states the data-write precondition: no two distinct write attempts can mutate
      the same key; the durability barrier is over immutable, attempt-unique objects.
- [ ] ADR §2 step 2 / §6.4 orphan story corrected: a fenced writer's staged objects are
      genuine orphans under a distinct key never referenced by a committed manifest.
- [ ] TLA+ model refined so a staged object's identity depends on the writer/attempt
      (e.g. `ObjId(v, w, k)` or a per-write token) -> two writers racing version v stage
      **distinct** ids; the winner's manifest references only its own ids.
- [ ] `OrphansNeverReferenced` invariant added (also satisfies formal-methods condition
      C-A) and the model re-checks clean (TLC; Apalache when available) — write-once
      immutability becomes a *checked* property, not an assertion.
- [ ] Integration test (when env exists): a zombie writer's late data PUT cannot corrupt
      a committed snapshot read (against the local S3 mock).
- [ ] Routed back to `steering-distributed-acid` (primary) + `steering-formal-methods`
      (model) + `steering-storage` (data-key layout / SPIKE-0003 cross-version sharing —
      content-addressing dovetails with their ref-counted-GC constraint) for confirm.
- [ ] `./format_code.sh` green.

## Notes / log
- 2026-06-13T20:20:00Z (steering-distributed-acid): filed from SPIKE-0005 primary
  verdict (decision 0023). This is a hard land-gate before commit-path tasks
  (T-0010 commit writer, T-0012 GC, T-0026 lease) become `ready`, and a
  pre-ratification obligation the SPIKE-0002 design gate must clear before I record the
  primary ratification. The SPIKE-0002 ADR + TLA+ model currently live on
  `work/SPIKE-0002-design-s3-commit-protocol-and-tla-model-for-atomic` (not yet on
  main); this fix should land with / as part of the SPIKE-0002 ratification cycle.
