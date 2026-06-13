---
id: SPIKE-0001
title: Define latency selectivity-envelope and analytical cost model
type: spike
status: in_review
priority: P0
assignee: researcher
epic: EPIC-003
deps: []
rubric_refs: [3, 11]
estimate: M
created: T0
updated: 2026-06-13T19:25:00Z
---

## Context

This spike is the **design-before-code gate** for all storage and query-execution work. The headline SLA (6-hop unanchored MATCH, 1B nodes / 10B edges, P99 ≤ 1 s cold at 1 Gbps) is achievable only conditionally — the physics bound what the engine can actually deliver. Before any implementation task in EPIC-001 or EPIC-002 is marked `ready`, this spike must be committed and steering-ratified.

The deliverable is a committed document/ADR that:

1. **Defines the selectivity envelope** with precise parameters:
   - Selectivity `s`: the fraction of nodes passing the node-property filter.
   - Byte budget `B_max`: derived as `bandwidth × (latency_budget − K·L_p99 − compute)`. Must show both cases — **1 Gbps ⇒ ~75 MB** and **50 Mbps ⇒ ~4 MB** (the binding constraint). `L_p99` is the S3 per-request P99 latency from published distributions (typ. 50–150 ms); `K` is the phase bound.
   - Phase bound `K`: the maximum number of sequential S3 round-trip phases allowed to serve one query. Derive a tight bound given the 6-hop structure and LIMIT-driven early termination.
   - Maximum allowed seed-set size: `|seed| ≤ B_max / (avg_node_bytes × avg_fan_out^6)` (or a tighter bound per the algebra).

2. **Proves the in-envelope claim**: an algebraic cost model showing that any query satisfying the envelope parameters can be served within B_max bytes over ≤ K sequential phases, hitting P99 ≤ 1 s. The proof must be self-contained — show each step.

3. **Specifies out-of-envelope detection**: what the planner must estimate at plan time (projected bytes-read, estimated fan-out), the threshold, and the required response (hard error, degraded plan, or explicit warning to caller). The detection algorithm must be O(plan-size) — it runs before any object-store access.

4. **References** the storage format constraints it imposes (fed into SPIKE-0003) and the latency distributions it assumes (to be validated by benchmark in EPIC-003).

This output is jointly a Cat. 3 (latency) and Cat. 11 (formal artifacts) deliverable. Steering sign-off: **steering-perf-sla** and **steering-formal-methods** must both approve before dependent tasks flip to `ready`.

## Acceptance criteria

- [ ] Envelope parameters (s, B_max, K, max seed-set size) defined with explicit derivations; the 1 Gbps ≈ 75 MB and 50 Mbps ≈ 4 MB cases both shown with the full formula.
- [ ] Analytical cost model committed: step-by-step algebra proving in-envelope queries hit P99 ≤ 1 s cold, citing S3 latency distribution assumptions.
- [ ] Out-of-envelope detection algorithm specified: input (query plan estimates), threshold formula, and required planner response documented.
- [ ] Document committed to `docs/design/` or `docs/adrs/` and cross-referenced from EPIC-003 and SPIKE-0003.
- [ ] Steering-ratification record committed: both steering-perf-sla and steering-formal-methods sign-off recorded in `.project/decisions/`.
- [ ] No code required — this is a design/proof artifact. A simulation to corroborate is welcome but not required here (that is EPIC-003's benchmark task).

## Notes / log

Output feeds directly into SPIKE-0003 (storage format spec must serve B_max) and into the TCK/planner implementation in EPIC-002 (planner must implement out-of-envelope detection). Steering sign-off unlocks those dependent tasks.

- **T0+~00:56 (2026-06-13T19:25:00Z):** researcher completed the envelope spec and cost model.
  - ADR committed: `docs/adr/0001-latency-selectivity-envelope.md`
  - Sign-off request: `.project/decisions/0012-spike-0001-steering-signoff-request.md`
  - All binding findings from ratification passes (decisions 0001, 0005, 0009, 0010 / BUG-0004, SPIKE-0006) incorporated:
    - M_max introduced as explicit envelope parameter; max-of-M α=1.10 at M_max=8.
    - K_min=8 (r=1), L_p99=50 ms named as explicit design-point parameters.
    - Invalid f^6 seed-set bound replaced with capped-frontier formulation.
    - Tail fan-out (not mean) from manifest statistics for estimator inputs.
    - Five OOE detection conditions with O(plan-size) algorithm.
    - Storage format constraints (r≤1, contiguous adjacency, manifest statistics, early-abort reads) fed to SPIKE-0003.
  - Status: in_review; awaiting steering-perf-sla + steering-formal-methods sign-off.
  - On ratification: T-0014, T-0015, T-0016 → ready; SPIKE-0003 constraints unblocked; SPIKE-0004 algebra unblocked.
