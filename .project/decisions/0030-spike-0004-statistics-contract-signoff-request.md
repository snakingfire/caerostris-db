# Decision 0030 — SPIKE-0004 manifest statistics contract: steering sign-off request

- **Date / marker:** 2026-06-13 (T0+~02:05)
- **Author / role:** `researcher` (SPIKE-0004 owner)
- **Type:** steering sign-off **request** (cross-cutting design artifact, ≥3-of-5 quorum)
- **Artifact:** `docs/specs/SPIKE-0004-manifest-statistics-contract.md`
- **Board item:** `SPIKE-0004` → `in_review` (awaiting ratification)
- **Status:** **REQUESTED — pending ≥3-of-5 quorum**
- **Rubric:** Cat. 3 (latency envelope, GATE, w14), Cat. 4 (planner, GATE, w12),
  Cat. 5 (index selectivity, w7); touches Cat. 2 (manifest, GATE, w12).
- **Owners (per `steering-committee.md`):** `steering-query-cypher` (primary —
  planner/selectivity), `steering-storage` (manifest layout), `steering-perf-sla`
  (envelope/detection). Cross-cutting ⇒ majority (≥3 of 5) ratifies.
- **Binds / discharges:** decision 0009 (planner stats + tail fan-out), decision
  0015 / ADR 0001 finding **F2** (per-rel-type **max** out-degree mandatory).

## What is being ratified

The exact statistics contract the query planner reads from the manifest to classify
a 6-hop query in/out of envelope before any object-store access. Summary (full
detail in the spec, Part 6):

1. **Statistics set** — per-label `node_count` + `total_node_count` (exact);
   per-(label,property) `ndv`/`null_frac`/MCV/`histogram` (selectivity); per-rel-type
   `edge_count`/`mean_deg`/`p99_deg`/**`max_deg`**/optional `degree_hist`; block
   metadata (`stats_version`/`as_of_version`/`freshness`/`estimator_params`).
2. **Home** — the immutable manifest. OOE-critical scalars (`node_count`,
   `total_node_count`, `edge_count`, `p99_deg`, `max_deg`) **inline** (zero extra
   GET); bulky per-property selectivity detail (MCV/histogram) may be a referenced
   content-addressed `db/stats/<hash>.stats` blob fetched lazily during planning for
   filtered properties only. Snapshot-consistent with the pinned version.
3. **Maintenance on commit** — exact counts + `max_deg` maintained incrementally
   per commit (`O(touched schema)`); p99/NDV/MCV/histogram recomputed on `ANALYZE`,
   carried forward with downgraded `freshness` between. Atomicity inherited from
   ADR 0002.
4. **Estimator** — two degree terms: `p99_deg` (typical byte estimate) and
   **`max_deg`** (super-hub safety gate). A query must clear both. Worked super-hub
   example: a 40M-degree celebrity node passes the p99 estimate but is rejected by
   the max-degree gate (2.56 GB single adjacency list ≫ 2.88 MB B_max) — the exact
   silent-SLA-miss F2 / decision 0015 demanded be caught at plan time.
5. **Missing/stale/absent rule** — conservative, never optimistic: absent
   selectivity ⇒ `s=1`; absent `max_deg` ⇒ `∞`; stale ⇒ degrade to sound bounds;
   default reject/warn; only escape is an explicit warning-emitting override.
   Freshly-ingested data with no stats ⇒ reject/warn + "run `ANALYZE`".
6. **License** — all recommended sketch crates (hyperloglogplus,
   probabilistic-collections, tdigest, blake3) are dual-permissive (Apache-2.0 OR
   MIT / CC0); in-tree implementation preferred; no copyleft/restricted dependency.

## Specific asks per owner

- **`steering-query-cypher` (primary):** confirm the statistics set and the
  selectivity-derivation (MCV + uniform-remainder + histogram interpolation) are
  sufficient for the planner's O(plan-size) seed-set sizing, and that the
  missing/stale doctrine (Part 3.3) matches the OOE-5 / decision 0009 stance.
- **`steering-storage`:** ratify the manifest home and the **inline-vs-referenced
  cut** (spec Part 2.1 (B) hybrid). The binding invariant to preserve:
  *super-hub/non-selective rejection needs no data-plane GET beyond the manifest.*
  Confirm the content-addressed `db/stats/<hash>.stats` blob fits the ADR 0002
  layout + GC reference-set rule, and the incremental-`max_deg` maintenance is
  compatible with the single-writer commit path. Fold the cut into SPIKE-0003.
- **`steering-perf-sla`:** confirm the two-term estimator (p99 typical / max-degree
  safety) discharges ADR 0001 F2 and that the super-hub example is correctly
  classified OOE; confirm the missing/stale rule cannot produce an optimistic accept.

## Conditions already folded in (not re-litigated)

- decision 0015 / ADR F2: `max_deg` is **mandatory**, not optional. ✓ (Part 1.3)
- decision 0009: tail/worst-case (not mean); missing ⇒ conservative reject. ✓ (Part 3)
- ADR 0001 §4.1/§4.3: O(plan-size), zero extra data-plane round-trip,
  snapshot-consistent. ✓ (Part 2.1)

## Alternatives considered (in the spec)

- **Mean degree only** — rejected (decision 0009/0015): silent SLA miss under
  power-law super-hubs.
- **p99 as the byte safety bound** — rejected (ADR F2): admits the super-hub.
  `max_deg` is the safety term.
- **Compute stats on demand at plan time** — rejected (decision 0009): adds
  data-plane round-trips to the latency budget.
- **Fully-inline vs. referenced stats blob** — kept open as the one storage call
  (spec R1); hybrid recommended; either is cost-model-safe.

## What unblocks on ratification

- `SPIKE-0004` → `done`.
- `T-0009` (manifest impl incl. statistics block) and `T-0015` (planner OOE
  detection) clear their `SPIKE-0004` dependency (T-0009 also needs SPIKE-0002 ✓,
  SPIKE-0003; T-0015 also needs SPIKE-0001 ✓, T-0009).
- `steering-storage` folds the inline-vs-referenced cut + `db/stats` blob layout
  into SPIKE-0003.

**Until ≥3-of-5 ratify:** the spec stays `proposed`; SPIKE-0004 stays `in_review`;
T-0009 / T-0015 stay `backlog` on their `SPIKE-0004` dep.
