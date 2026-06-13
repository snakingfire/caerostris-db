---
id: SPIKE-0004
title: Planner out-of-envelope detection needs maintained graph statistics in the manifest
type: spike
status: backlog
priority: P0
assignee:
epic: EPIC-003
deps: [SPIKE-0001]
rubric_refs: [3, 4, 5]
estimate: M
created: 2026-06-13T18:24:00Z
updated: 2026-06-13T18:24:00Z
---

## Context

Filed by `steering-query-cypher` during the launch ratification pass over
`docs/commanders-intent.md` and `docs/requirements/master-rubric.md`. **Joint
steering item** (steering-query-cypher + steering-storage + steering-perf-sla).

The single non-negotiable (commanders-intent.md L62, L101): out-of-envelope
queries must be **detected at plan time and never silently miss the SLA**. R7 and
SPIKE-0001 assign this to the planner: it must estimate "projected bytes-read,
estimated fan-out" in `O(plan-size)` **before any object-store access**, then
reject/warn/degrade.

**The gap:** a sound out-of-envelope estimate for a 6-hop expansion needs
real inputs the design currently sources from nowhere:

- **Per-property / per-label selectivity** `s` — to size the seed set after the
  anchoring filter. Without statistics the planner cannot know whether
  `WHERE n.name = 'X'` returns 1 node or 10^8.
- **Per-relationship-type average and tail (e.g. p99) fan-out / degree** — to
  bound frontier growth across 6 hops. A mean degree is not enough: a single
  super-node (power-law degree, which 1B/10B real graphs have) busts B_max even
  when the *average* is benign. The estimate must use a tail bound, not the mean,
  or it will under-estimate and silently blow the SLA — the exact failure the
  invariant forbids.

These statistics must be **maintained by the storage layer and published in the
manifest** (so the planner reads them with the version it pinned — no extra
round-trips, snapshot-consistent). SPIKE-0001 (cost model), SPIKE-0003 (storage
format spec), and EPIC-002 (planner) each assume this but **none names the
statistics contract**. Without it, "estimate bytes/fan-out" is a guess, and an
optimistic guess is a silent SLA miss.

**Does NOT block launch.** SPIKE-0001 can be ratified for the *envelope algebra*;
this spike pins the *estimator inputs* that the algebra consumes and must be
ratified before the planner's detection code (EPIC-002) and the manifest
statistics (EPIC-001 / SPIKE-0003) move to `in_progress`.

## Acceptance criteria

- [ ] A statistics contract is specified: the exact set the planner needs
      (per-label node counts; per-property value cardinality/histogram or a
      selectivity estimate; per-rel-type total + degree distribution summary
      incl. a tail/p99 or max-degree term).
- [ ] Where each statistic lives (manifest vs. index metadata vs. side files),
      how it is maintained on commit, and that readers see it consistently with
      their pinned snapshot — agreed with `steering-storage` and folded into
      SPIKE-0003.
- [ ] The out-of-envelope estimator is restated to use a **tail/worst-case**
      fan-out bound (not the mean), with the super-node case worked as an example
      that the planner correctly classifies as out-of-envelope.
- [ ] What the planner does when a needed statistic is **missing/stale** (e.g.
      freshly ingested data with no stats yet): default to conservative reject/warn,
      never optimistic accept. This rule is written down.
- [ ] Cross-references added from SPIKE-0001, SPIKE-0003, EPIC-002, EPIC-005.
- [ ] Steering sign-off: steering-query-cypher + steering-perf-sla +
      steering-storage (≥3-of-5 quorum, cross-cutting) recorded in
      `.project/decisions/`.
- [ ] No code (design/spec artifact).

## Notes / log
- T0 `steering-query-cypher`: filed during ratification. Decision recorded at
  `.project/decisions/0009-planner-stats-and-tail-fanout-bound.md`. This is the
  bridge between Cat. 3 (envelope), Cat. 5 (index selectivity) and Cat. 4 (planner).
- **T+~01:28 steering-formal-methods condition (decision 0015, ADR 0001 finding F2):**
  the per-rel-type degree statistic MUST include the **max** out-degree (not only p99/tail).
  Rationale: ADR §2.2's per-hop byte bound uses `F_tail` as if it were a hard per-node cap,
  but a p99 admits the ~1% of super-hub nodes above it; a single super-hub adjacency list
  (out-degree 10⁵–10⁸ → 6 MB–6.4 GB) busts B_max. The estimator's byte SAFETY bound must
  use the max-degree (or a hard early-abort per-GET byte cap), so tighten the statistics
  contract from "incl. a tail/p99 or max-degree term" to "incl. BOTH a p99/tail term AND a
  per-rel-type max out-degree". This is a binding input for T-0015.
