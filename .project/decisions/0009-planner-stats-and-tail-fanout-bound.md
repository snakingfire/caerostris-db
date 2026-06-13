# Decision 0009 — Out-of-envelope detection needs maintained statistics and a tail fan-out bound

- **Date / marker:** T0 (2026-06-13T18:24:00Z)
- **Owner:** steering-query-cypher (joint: steering-perf-sla, steering-storage)
- **Status:** recorded; tracked by SPIKE-0004 (P0)
- **Rubric:** Cat. 3 (latency envelope), Cat. 4 (planner), Cat. 5 (index selectivity)

## Context

The non-negotiable invariant: out-of-envelope queries are detected at plan time
and never silently miss the SLA (commanders-intent.md L62, L101). R7/SPIKE-0001
assign the planner the job of estimating projected bytes-read and fan-out in
`O(plan-size)` before any object-store access.

## Finding

A sound estimate for a 6-hop expansion requires inputs the design sources from
nowhere:

1. **Per-property/label selectivity** to size the post-filter seed set.
2. **Per-relationship-type degree distribution with a tail term** (p99 or
   max-degree). A *mean* degree is unsafe: real 1B/10B graphs are power-law, and a
   single super-node busts B_max even when the average is benign. An
   estimator using the mean will under-estimate and silently blow the SLA — the
   precise failure the invariant forbids.

These statistics must be maintained by storage and published in the manifest so
the planner reads them consistently with its pinned snapshot. SPIKE-0001,
SPIKE-0003, and EPIC-002 each assume this but none names the contract.

## Decision

- File SPIKE-0004 (P0, joint steering) to pin: (a) the statistics set, (b) where
  they live and how they are maintained on commit (snapshot-consistent), (c) the
  estimator must use a **tail/worst-case** fan-out bound, (d) the
  missing/stale-statistics rule = conservative reject/warn, never optimistic
  accept.
- SPIKE-0001 may be ratified for the envelope *algebra*; SPIKE-0004 pins the
  estimator *inputs* and must ratify before the planner detection code (EPIC-002)
  and manifest statistics (EPIC-001/SPIKE-0003) move to `in_progress`.

## Alternatives considered

- **Estimate fan-out from mean degree only.** Rejected: unsafe under power-law
  degree (super-nodes) — silent SLA miss.
- **Compute statistics on demand at plan time.** Rejected: that is object-store
  access before the O(plan-size) pre-check, defeating the purpose and adding
  round-trips to the latency budget.

## Consequences

A statistics contract threads Cat. 3 (envelope), Cat. 5 (index selectivity), and
Cat. 4 (planner). Folded into SPIKE-0003 and cross-referenced from SPIKE-0001,
EPIC-002, EPIC-005.
