# Decision 0032 — steering-storage ratification of SPIKE-0003 (on-object storage format spec)

- **Date / mark:** 2026-06-13 (T0+~03:40)
- **Author:** `steering-storage` (design authority, rubric Cat. 2; owner of decision
  0001 findings F1/F2/F3)
- **Type:** Storage-format ADR ratification (design-falsification loop run by the
  authoring authority before sign-off, per `docs/process/adversarial-review-loops.md`
  and `docs/process/steering-committee.md`).
- **Artifact:** `docs/adr/0008-storage-format.md` (drafted as 0006; renumbered to
  0008 at ratification to avoid collision with the in-flight Python-bindings ADR
  (T-0030), which had claimed 0006/0007 on its PR branch — BUG-0010 collision class
  avoided).
- **Verdict:** **APPROVE (ratified-with-conditions).** ADR status → `accepted`;
  SPIKE-0003 → `done`.
- **Status:** accepted

## Context

SPIKE-0003 is the keystone storage-format spec blocking the entire storage cascade
(T-0007 columnar node-property writer/reader, T-0008 adjacency-list edge
writer/reader, T-0009 manifest + statistics + version resolution, T-0010 atomic
commit) — i.e. Cat. 2 (storage, w12), the substrate for Cat. 1 (ACID, w14), and the
executor for Cat. 4. It was authored and ratified in a single pass by
`steering-storage` (the design authority) under pace-marshal P0 keystone authority.

The spec was constrained by, and falsification-tested against, every pre-registered
upstream constraint:
- **ADR 0001** (latency envelope) Part 5 #1–#5 + PS-1 (`r ≤ 1`, contiguous
  adjacency, columnar node properties, manifest statistics, early-abort byte cap,
  `K_min` stays 8).
- **ADR 0002** (commit protocol) §1/§6 + decision 0027 BC-1 (content-addressed
  write-once keys, create-only-CAS, durability barrier, reference-counted GC, pins).
- **decision 0001** F1/F2/F3 (SPIKE-0008): early-abort, named CAS primitive, safe
  GC vs. readers.
- **SPIKE-0004** R1 (`docs/specs/SPIKE-0004-manifest-statistics-contract.md`): inline
  OOE-critical scalars + referenced stats blob; value-digest privacy (BLAKE3-trunc,
  never raw values).

## Decision

The format adopts a **content-addressed, columnar-node (`.ncol`) / CSR-adjacency
(`.adj`) / partition-mapped-manifest** layout:

1. **Node properties:** columnar, label-partitioned, sorted by node id into ≤ ~4 MiB
   id-band shards; per-object column directory so a filter reads only its column's
   byte range. (ADR §2.)
2. **Adjacency:** banded CSR per `(rel-type, direction)`, sorted by source id, with
   an **intra-object fixed-stride offset directory** (source-id → block byte range +
   degree) and a **co-located projection set** (filtered + returned hot dst
   properties). (ADR §3.)
3. **`r ≤ 1`:** the manifest **partition map** (read once, phase 1) + the
   intra-object offset directory let the reader compute every hop-`h+1` neighbor-
   block address with no intervening GET; the directory slice is fetched in the same
   parallel batch as the neighbor blocks. `K_min = 8` preserved. (ADR §4.)
4. **PS-1:** returnable projection co-located in the hop-6 block (no 9th serial
   phase); explicit manifest-recorded `K_min = 9` fallback when a projection set is
   too wide. (ADR §3.3, §7.3.)
5. **Early-abort = hard per-GET byte/row cap** driven by the offset directory's
   `block_len`/`degree`, bounding realized super-hub reads (F1/F2). (ADR §3.4.)
6. **Manifest:** root object with exact `objects[]` reference set + partition map +
   inline OOE-critical stats (`node_count`, `total_node_count`, `edge_count`,
   `p99_deg`, **`max_deg`**) + referenced `db/stats/<hash>.stats` blobs;
   value-digest privacy. (ADR §5.)
7. **Versioning / commit / GC:** inherited from ADR 0002 — content-addressed keys,
   create-only-CAS manifest swap, durability barrier, **reference-counted live-
   object-set GC** (satisfies decision 0027 BC-1 by construction). (ADR §7.)
8. **Schema evolution:** add-column = new chunk in new shards only; absent column =
   null via self-describing directory; readers **fail-closed** on unknown
   version/codec. (ADR §8.)

## Falsification summary (why ratify, not bounce)

Seven attacks run; all survived with cited structural evidence (full record in ADR
§Sign-off): (1) `r` secretly 2 → refuted by intra-object offset directory + manifest
map; (2) PS-1 K→9 → refuted by co-located projection + explicit K=9 fallback; (3)
super-hub realized bytes → refuted by `max_deg` inline detection + hard per-GET byte
cap; (4) selective-scatter → refuted by sorted bands + variable-width banding; (5)
torn-read/GC-mid-read → refuted by ADR 0002 inheritance (NoTornCommit/GCSafety/
OrphansNeverReferenced over content-addressed exact-`objects[]`); (6) hand-wavy byte
budget → refuted by §6 reducing exactly to ADR 0001 §2.2's inequality + worked
1.31 MB ≤ 2.88 MB 50 Mbps point; (7) schema-evolution fail-open → refuted by
self-describing footer + fail-closed versioning (BUG-0014 lesson). No attack moved
the feasible region or contradicted an upstream ratified ADR ⇒ ratify-with-
conditions is correct under pace doctrine (Cat. 2 GATE, keystone cascade).

## Conditions (binding on dependent tasks — land-gates, not design blockers)

- **C1 (T-0008, T-0018):** `r ≤ 1` is a tested invariant — adjacency reader computes
  hop-`h+1` addresses from manifest map + offset directory with no intervening GET;
  integration test asserts ≤ `M_max` GETs/hop. If `r = 2` is unavoidable, escalate
  to steering (do not ship `K_min = 14`).
- **C2 (T-0008):** early-abort = hard per-GET byte/row cap, consulting offset
  directory before fetching neighbor bytes; test shows a super-hub frontier does not
  read beyond the cap.
- **C3 (T-0007):** columnar filter read touches only the filtered column's chunk,
  not whole node records (asserted on the mock).
- **C4 (T-0009):** inline OOE-critical scalars + referenced MCV/histogram blob;
  value-digest privacy (BLAKE3-trunc, never raw values); test asserts no raw value in
  any committed manifest/stats fixture.
- **C5 (T-0009/T-0010):** exact `objects[]` reference set + reference-counted live-
  object-set GC (decision 0027 BC-1); inherits ADR 0002 mock-fidelity CAS test (two
  concurrent `PUT If-None-Match:*` → one 200 / one 412).
- **C6 (T-0009 + planner):** the `K_min = 9` fallback is explicit and manifest-
  recorded; planner re-pins budget + OOE thresholds for plans over a rel-type with
  `colocated_projection = false`. Default = co-located `K_min = 8`.

## Non-blocking notes

- **N1:** partition-map externalization (`db/manifest-index/<hash>`) reserved for
  >10× the design point (ADR §8.2); per-query read is a KB slice via segmented binary
  search at the design point.
- **N2:** binary manifest sidecar behind `format_version` is a future perf option
  (T-0016 may quantify), not a break.
- **N3:** projection-set co-location heuristic is open tuning; format correct for any
  choice.

## Consequences

- ADR 0008 `accepted`; SPIKE-0003 `done`.
- **Unblocks** T-0007, T-0008, T-0009, T-0010 (their `SPIKE-0003` dep clears; T-0009
  also needs SPIKE-0002 `done` (✓) and SPIKE-0004 ratified — track that separately).
  Conditions C1–C6 attach to those tasks as land-gates.
- Cat. 2 (storage, GATE, w12) advances toward 100: a concrete, falsification-survived
  format spec now exists.
- Satisfies decision 0027 BC-1 (cross-version sharing → reference-counted GC) by
  construction, closing the constraint `steering-storage` placed on SPIKE-0003 at
  ADR 0002 ratification.

## Cross-references

- `docs/adr/0008-storage-format.md` (the ratified artifact).
- `docs/adr/0001-latency-selectivity-envelope.md` (Part 5, PS-1 — discharged).
- `docs/adr/0002-s3-commit-protocol.md` (§1/§6 — re-pointed).
- `docs/specs/SPIKE-0004-manifest-statistics-contract.md` (R1 — made binding).
- `.project/decisions/0001-storage-domain-ratification-findings.md` (F1/F2/F3).
- `.project/decisions/0027-storage-domain-signoff-spike-0002-commit-protocol.md`
  (BC-1 — satisfied).
- `.project/board/tasks/SPIKE-0003-storage-format-spec.md` (→ done).
