# Decision 0012 — SPIKE-0001 Steering Sign-off Request: Latency Selectivity-Envelope ADR

- **Date:** 2026-06-13T19:20:00Z (T0+~00:56)
- **Author:** researcher (SPIKE-0001)
- **Type:** steering sign-off request (design-level)
- **Status:** in_review — awaiting ratification from steering-perf-sla and
  steering-formal-methods
- **Artifact:** `docs/adr/0001-latency-selectivity-envelope.md`
- **Rubric:** Cat. 3 (latency envelope + SLA, GATE, w14), Cat. 11 (formal artifacts, GATE, w6)
- **Dependent tasks that unblock on ratification:** T-0014, T-0015, T-0016, SPIKE-0003
  (constraints), SPIKE-0004 (statistics contract)

## What was produced

`docs/adr/0001-latency-selectivity-envelope.md` contains:

1. **Part 1 — Envelope parameter definitions** with explicit derivations:
   - Named parameters: `T_budget`, `W`, `L_p99`, `r`, `K_min`, `T_compute`, `s`, `F_tail`,
     `M_max` — all first-class envelope parameters.
   - Byte budget `B_max` derived for both bandwidth cases (1 Gbps ≈ 57.5 MB;
     50 Mbps ≈ 2.88 MB) using the M_max-corrected latency term.
   - Phase depth `K_min = 1 + 1 + 6·r = 8` (at r=1) derived from the cold-start
     query structure, not asserted.
   - L_p99 sensitivity table showing the 2 s ceiling survives up to L_p99 = 237 ms
     (at K_min=8, T_compute=100 ms).
   - M_max=8 chosen with max-of-M amplification factor α=1.10 (from decision 0005
     Monte-Carlo).

2. **Part 2 — Selectivity envelope and seed-set bound**, replacing the invalid `f^6`
   cartesian-expansion formula with the capped-frontier formulation:
   - LIMIT-driven early termination and frontier width cap M_max are the operative
     mechanisms (not the cartesian product).
   - Seed-set bound expressed as: `N_seed × bytes_node + 6 × M_max × F_tail × bytes_edge_row ≤ B_max`.
   - Selectivity `s_max` derived for the 50 Mbps binding case: ≈ 1.1×10^-5 (1-in-100,000 nodes).

3. **Part 3 — Analytical cost model and SLA proof**:
   - Full formula: `T_query(P99) = T_lat + T_transfer + T_compute`.
   - Step-by-step proof for both the 1 Gbps and 50 Mbps cases showing
     `T_query ≤ T_budget = 1.000 s` at the envelope boundary.
   - 50 Mbps case is tight (1.001 s at the boundary); conditions that make it
     feasible are all named explicitly.

4. **Part 4 — Out-of-envelope detection algorithm**:
   - Five detection conditions: OOE-1 (no LIMIT), OOE-2 (bytes > B_max), OOE-3
     (frontier width), OOE-4 (deployment too slow), OOE-5 (missing statistics).
   - Algorithm complexity: O(plan-size + statistics-lookups).
   - Required planner responses per condition (hard error vs. warning vs. override).

5. **Part 5 — Storage format constraints** fed to SPIKE-0003:
   - r ≤ 1 (hard constraint)
   - Contiguous adjacency layout
   - Columnar node-property layout
   - Manifest statistics (fed to SPIKE-0004)
   - Early-abort adjacency reads

6. **Part 6 — Latency distribution assumptions and calibration hook** for T-0014.

## Prior findings incorporated

All binding findings from the ratification passes have been folded in:

| Finding | Source | Incorporated where |
|---------|---------|-------------------|
| Intra-phase max-of-M order statistic | decision 0005 / BUG-0004 | §1.7, Part 3 cost model, Part 4 OOE-3 |
| Serial K_min latency floor | decision 0010 / SPIKE-0006 | §1.3, §1.4, Part 3 |
| L_p99 as named parameter with explicit assumed value | SPIKE-0006 | §1.4 |
| r ≤ 1 storage constraint | decision 0010 | §1.5, Part 5 |
| 2 s ceiling worst-case L_p99 | decision 0010 | §1.4 |
| Invalid f^6 cartesian bound → capped frontier | SPIKE-0006 note | §2.1, §2.2 |
| Tail/worst-case fan-out from manifest stats | decision 0009 / SPIKE-0004 | §2.2, Part 4 §4.1 |
| Conservative reject when statistics missing | decision 0009 | Part 4 §4.5 (OOE-5) |
| F1: LIMIT + early-abort adjacency mandatory | decision 0001 / SPIKE-0008 | §2.3, Part 5 |
| Bandwidth ambiguity: prove both cases analytically | decision 0010 | §1.2 (both cases shown), §3.2–3.3 |

## Remaining open questions (not blocking ratification)

1. α calibration: the α=1.10 value (max-of-M amplification at M_max=8) will be
   validated by T-0014 (discrete-event simulation on the mock). If empirical α > 1.10,
   B_max decreases and the envelope tightens; the detection algorithm (T-0015) must be
   updated. This is a calibration task, not a design-level unknown.
2. SPIKE-0003 must confirm r ≤ 1 is achievable. If not, escalate to steering immediately.
3. SPIKE-0004 must ratify the statistics contract before T-0015 is implemented.

## Request to steering

**steering-perf-sla** and **steering-formal-methods** are requested to:

1. Adversarially review `docs/adr/0001-latency-selectivity-envelope.md` against their
   respective mandates (latency feasibility; formal artifact correctness).
2. Append a sign-off entry to the ADR's Sign-off section (per `docs/adr/README.md`).
3. If changes are requested, this decision record will be updated with the outcome
   and the ADR revised accordingly.

**Ratification unblocks:**
- T-0014, T-0015, T-0016 → move from `backlog` to `ready`
- SPIKE-0003 (storage format spec) → its constraints are now defined
- SPIKE-0004 (statistics contract) → the estimator algebra it must satisfy is defined

**SPIKE-0001 status after ratification:** `done`
