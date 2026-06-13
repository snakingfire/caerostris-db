---
id: SPIKE-0001
title: Define latency selectivity-envelope and analytical cost model
type: spike
status: done
priority: P0
assignee: researcher
epic: EPIC-003
deps: []
rubric_refs: [3, 11]
estimate: M
created: T0
updated: 2026-06-13T21:58:00Z
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
- [ ] Document committed to `docs/adr/` and cross-referenced from EPIC-003 and SPIKE-0003.
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

- **T+~01:28 (2026-06-13T19:52:00Z) steering-formal-methods — RATIFIED-WITH-CONDITIONS (secondary sign-off):**
  - Decision: `.project/decisions/0015-formal-methods-spike-0001-ratification.md`.
  - Sign-off appended to the ADR Sign-off section (`docs/adr/0001-latency-selectivity-envelope.md`).
  - **The latency theorem CLOSES** — I independently re-derived every figure (B_max both
    bandwidths, T_lat=440 ms, both boundary T_query, seed-set/s_max points); all match. No
    escalation to the full committee.
  - **2 findings bound as conditions on dependent tasks (not blocking this ratification):**
    - **F1** — α dropped from ADR §1.4 ceiling sensitivity and OOE-4 thresholds; the
      α-corrected, self-consistent thresholds are **102 ms (1 s) / 216 ms (2 s ceiling)**.
      Bound to **T-0015**.
    - **F2** — §2.2 uses p99 `F_tail` as a hard per-node cap; a super-hub (out-degree ≫ p99)
      busts B_max in one GET. Realized SLA is protected by early-abort; the estimator must use
      the per-rel-type **max** degree (or a hard per-GET byte cap), not p99. Bound to
      **SPIKE-0004** (maintain max-degree) + **T-0015** (estimator) + **SPIKE-0003** (early-abort
      as a hard cap).
  - **F3** (non-blocking): ADR-0001 numbering collision filed as **BUG-0010**.
  - **REMAINING GATE — quorum:** latency-envelope params need steering-perf-sla (primary) +
    steering-formal-methods (secondary). perf-sla pre-approved the framing in decision 0010
    (all findings folded in) but has **not** counter-signed the *committed ADR*. Per the honest
    two-signature rule, this item stays `in_review`; ADR stays `proposed`; T-0014/15/16 do NOT
    flip to `ready` until perf-sla appends its counter-signature to the ADR Sign-off section.
  - **Action requested:** `steering-perf-sla` review F1/F2 and counter-sign the ADR. On that
    second signature: ADR → `accepted`; this item → `done`; planner/integrator flips
    T-0014, T-0015, T-0016 → `ready` (SPIKE-0006 already `done`); SPIKE-0003 (in_progress) and
    SPIKE-0004 proceed with conditions F1/F2 attached.

- **T+~01:58 (2026-06-13T21:58:00Z) steering-perf-sla — RATIFIED-WITH-CONDITIONS (PRIMARY sign-off):**
  - Decision: `.project/decisions/0017-perf-sla-spike-0001-ratification.md`.
  - Counter-signature appended to the ADR Sign-off section (Round 2). **Quorum now 2-of-2
    complete** (perf-sla primary + formal-methods secondary).
  - **ADR `docs/adr/0001-latency-selectivity-envelope.md` → `accepted`. This item → `done`.**
  - Independently re-derived every load-bearing figure (B_max both bandwidths, T_lat=440 ms,
    boundary T_query=1.000 s both cases, seed/s_max bounds, F1 thresholds 102/216 ms, F2
    super-hub busts) — all match the ADR and decision 0015. Ran 4 falsification attacks
    (benchmark↔cost-model coherence, worst-case in-envelope query, F2 realized-latency,
    hidden-serial-phase) — all survived. The latency theorem closes; no escalation.
  - **Conditions bound to dependent tasks (not blocking this ratification):**
    - **F1** → T-0015: α-corrected OOE-4 thresholds 102 ms (1 s) / 216 ms (2 s).
    - **F2** → SPIKE-0004 (per-rel-type max out-degree) + T-0015 (max-degree byte bound,
      super-hub reject) + SPIKE-0003 (early-abort as hard per-GET byte/row cap).
    - **PS-1** → SPIKE-0003: co-locate filter/return node properties with hop-6 adjacency to
      keep K_min=8, OR declare K_min=9 and re-pin SPIKE-0001's B_max / OOE thresholds before
      T-0015/T-0016 consume them.
    - **PS-2** → T-0016: Cat. 3 measured evidence MUST be cache-OFF, fresh-state-per-sample,
      named profile (nominal-s3/slow-s3), N ≥ 200 per the cold-start-benchmark-protocol ADR;
      cache-on/loopback/fast-s3 results are a reject.
  - **Unblocks:** T-0014, T-0015, T-0016 eligible to flip `ready` (planner/integrator next
    grooming pass); SPIKE-0003 (in_progress) and SPIKE-0004 proceed against Part 5 constraints
    with F1/F2/PS-1 attached.

- **T+~04:05 SPIKE-0004 closed the estimator-inputs gap (`docs/specs/SPIKE-0004-manifest-statistics-contract.md`):**
  ADR 0001 §4.1's "manifest statistics (SPIKE-0004)" are now pinned to an exact contract —
  per-label `node_count`/`total_node_count`, per-(label,property) selectivity (NDV/MCV/histogram),
  and per-rel-type degree summary with BOTH `p99_deg` (typical) AND the mandatory `max_deg`
  (super-hub safety gate, discharging finding F2 / decision 0015). The F2 condition this ADR
  bound to SPIKE-0004 is satisfied. Sign-off request:
  `.project/decisions/0030-spike-0004-statistics-contract-signoff-request.md` (≥3-of-5 quorum).
