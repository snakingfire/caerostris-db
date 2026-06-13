---
id: SPIKE-0004
title: Planner out-of-envelope detection needs maintained graph statistics in the manifest
type: spike
status: in_review
priority: P0
assignee: researcher
epic: EPIC-003
deps: [SPIKE-0001]
rubric_refs: [3, 4, 5]
estimate: M
created: 2026-06-13T18:24:00Z
updated: 2026-06-13T20:05:00Z
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

## Research: what statistics must the storage layer maintain in the manifest so the planner can classify a 6-hop query in/out of envelope (using a tail/worst-case, not mean, fan-out) before any object-store access, and what does it do when a statistic is missing/stale?

- **T+~02:05 `researcher` — SPIKE COMPLETE, routed to steering (cross-cutting, >=3-of-5 quorum).**
  Full spec: **`docs/specs/SPIKE-0004-manifest-statistics-contract.md`**.
  Sign-off request: **`.project/decisions/0030-spike-0004-statistics-contract-signoff-request.md`**.
  Board item set to `in_review` pending ratification (NOT `done` — this is a cross-cutting
  design artifact that gates T-0009 + T-0015 and must be ratified next round).

  **Statistics contract (the answer):**
  - **Per-label:** `node_count[l]` + `total_node_count` (exact, incremental on commit;
    doubles as Cat. 6 fast `count`).
  - **Per-(label,property) selectivity:** `ndv` (HyperLogLog), `null_frac`, `most_common`
    (top-H MCV list, captures skew), `histogram` (equi-depth quantiles for range/prefix).
    Values stored as fixed-width **digests**, never raw — bounds size and keeps the public
    repo free of user data by construction (guardrails §3).
  - **Per-rel-type degree (both directions):** `edge_count`, `mean_deg` (diagnostics only —
    never the estimator), `p99_deg` (typical fan-out), and **`max_deg` (MANDATORY,
    decision 0015 / ADR 0001 F2 — the super-hub safety term)**, optional `degree_hist`.
  - **Block metadata:** `stats_version`, `as_of_version`, `freshness` (exact/estimated/
    stale/absent per family), `estimator_params`.

  **Home & maintenance (with steering-storage):** in the immutable manifest (ADR 0002 §1) so
  the version pin and the stats are the **same read** — snapshot-consistent, zero extra
  data-plane round-trip, works in master-less/read-only modes. OOE-critical scalars
  (`node_count`, `total_node_count`, `edge_count`, `p99_deg`, `max_deg`) **inline** (~1 KB,
  under the cost model's 4 KB manifest reserve); bulky per-property MCV/histogram detail may
  be a referenced content-addressed `db/stats/<hash>.stats` blob fetched lazily only for
  filtered properties. Exact counts + `max_deg` maintained **incrementally** per commit
  (`O(touched schema)`, sound upper bound on deletes); p99/NDV/MCV/histogram recomputed on
  `ANALYZE`, carried forward with downgraded `freshness` between. Atomicity inherited from
  the commit protocol (no separate stats-durability mechanism). The inline-vs-referenced cut
  is steering-storage's final call (spec R1); the invariant to preserve is "super-hub /
  non-selective rejection needs no data-plane GET beyond the manifest."

  **Estimator restated with a tail/worst-case bound (discharges ADR F2):** TWO degree terms
  with different jobs — `p99_deg` sizes the *typical* byte estimate; **`max_deg`** (or the
  storage per-GET byte cap) is the **super-hub safety gate**. A query must clear BOTH. Worked
  example: a `FOLLOWS` rel-type with `p99_deg=120` but `max_deg=4.0e7` (a 40M-follower
  celebrity) — a tiny seed set's p99 byte estimate is ~1.65 MB <= 2.88 MB (looks in-envelope),
  but the single max-degree adjacency list is 2.56 GB >> B_max ⇒ the planner classifies it
  **out-of-envelope** on the max-degree gate. p99 alone is FORBIDDEN as the safety bound.

  **Missing/stale/absent rule (written down, binding):** *the planner never makes an
  optimistic assumption from a statistic it does not trust.* Absent selectivity ⇒ `s=1` ⇒
  OOE-5 reject; absent `max_deg` ⇒ `∞` ⇒ super-hub gate fails ⇒ reject; stale ⇒ degrade to
  sound bounds (incremental `max_deg` upper bound is still safe). Default reject/warn; the
  only escape is an explicit, warning-emitting override (`SET envelope_check=WARN`,
  `ALLOW_MISSING_STATS`) — never silent accept. **Freshly-ingested data with no stats yet**
  is conservatively rejected/warned with a "run `ANALYZE`/`REFRESH STATISTICS`" message — the
  invariant is asymmetric: better to reject a maybe-in-envelope query than accept a
  maybe-SLA-busting one.

  **License check (mandatory):** all recommended estimator crates are dual-permissive —
  `hyperloglogplus` (Apache-2.0 OR MIT), `probabilistic-collections` (Apache-2.0 OR MIT),
  `tdigest` (Apache-2.0 OR MIT), `blake3` (CC0-1.0 OR Apache-2.0). **No GPL/AGPL/SSPL/BUSL
  or distribution-restricted dependency is needed.** Recommendation: prefer **in-tree**
  sketch implementations (minimal surface); the above are vetted permissive fallbacks.
  Final `cargo deny check licenses` at add-time (guardrails §5; BUG-0008 AND/OR precedence —
  all are OR-conjunctions ⇒ permissive).

  **Cross-references threaded by this commit:** ADR 0002 §1 "SPIKE-0009" manifest-stats
  reference corrected to **SPIKE-0004** (was a stale id — SPIKE-0009 is the server-mode
  network protocol). T-0009 statistics-block wording aligned to this contract (adds
  `max_deg`). Spec cross-refs ADR 0001 (F2), ADR 0002, SPIKE-0003, EPIC-002/T-0015,
  EPIC-005, EPIC-001/T-0009, Cat. 6.

  **Next step (planner):** on >=3-of-5 ratification of decision 0030, flip SPIKE-0004 → `done`;
  T-0009 and T-0015 clear their `SPIKE-0004` dep; steering-storage folds the
  inline-vs-referenced cut + `db/stats` blob layout into SPIKE-0003. No new tasks needed —
  T-0009 (manifest stats block) and T-0015 (planner OOE detection) already exist and own the
  implementation.

  **Confidence:** high. Every acceptance-criterion bullet and every ADR 0001 F2 / decision
  0015 condition is discharged; the one genuinely open knob (R1, inline-vs-referenced) is a
  storage-layout refinement that does not move the feasible region.
