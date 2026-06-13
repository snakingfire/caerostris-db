---
id: BUG-0001
title: Latency byte-budget formula under-reserves intra-phase tail (max-of-M parallel GETs)
type: bug
status: ready
priority: P0
assignee:
epic: EPIC-003
deps: []
rubric_refs: [3, 11]
estimate: M
created: 2026-06-13T18:30:41Z
updated: 2026-06-13T18:30:41Z
---

## Context

Ratification-pass finding by `steering-formal-methods` against
`docs/commanders-intent.md` (latency theorem) and `docs/requirements/master-rubric.md`
(Cat. 3) / `core-requirements.md` R7 / `formal-verification-policy.md`.

The byte-budget formula is stated everywhere as:

    B_max = W × (T_budget − K·L_p99 − T_compute)

This reserves `K · L_p99` for latency, i.e. it treats the per-phase wait as the
*per-request* P99 (`L_p99`) in each of the K sequential phases. That is **optimistic**
for the access pattern the design itself mandates: "few, large, **parallel** range GETs"
over an expanding frontier. A phase that issues M parallel GETs does not complete at the
per-request P99 — it completes when the **slowest of M** GETs returns (a max-of-M order
statistic), whose tail is materially worse than a single GET's tail.

Adversarial simulation (lognormal GET, P50=20 ms, P99=100 ms; reproducible, see decision
0001) — query-P99 of the SUM of K phases, each = max-of-M parallel GETs, vs the documented
reservation `K·L_p99`:

| K | M (parallel GETs/phase) | query P99 | naive K·L_p99 | ratio |
|--:|------------------------:|----------:|--------------:|------:|
| 3 | 1   | 193 ms  | 300 ms | 0.64 |
| 3 | 8   | 332 ms  | 300 ms | 1.11 |
| 3 | 64  | 527 ms  | 300 ms | 1.76 |
| 3 | 256 | 693 ms  | 300 ms | 2.31 |
| 5 | 64  | 790 ms  | 500 ms | 1.58 |
| 5 | 256 | 1034 ms | 500 ms | 2.07 |

Two effects compete: across-phase summing *concentrates* the tail (CLT, pulls the ratio
< 1 at M=1), but within-phase max-of-M *amplifies* it. For the realistic regime (high
fan-out: tens-to-hundreds of parallel range GETs per frontier phase) amplification wins:
the actual query P99 can be ~2× the latency the formula reserves, and at K=5/M=256 the
**latency term alone (1034 ms) blows the 1 s target before a single transferred byte is
counted.**

**Severity / why P0, but NOT a launch-blocker.** The theorem still *closes*: there is a
clear feasible parameter region (K=3, modest M, L_p99≈50–100 ms gives 50 Mbps≈4 MB /
1 Gbps≈75–100 MB with the latency tail accounted for, and everything stays under the 2 s
ceiling). So this is **not** a falsification of the design — it is a precise
under-specification in the artifact the rubric grades against. If SPIKE-0001 derives
`B_max` from the documented formula verbatim, the analytical model will under-reserve
latency, and the sim/benchmark may then miss the SLA with no anchor in the spec to catch
why. That is exactly the "fast only with luck" trap the commander's intent forbids, hiding
inside the cost-model algebra. Surfaced-and-tracked per the ratification doctrine
(we do not hard-block the launch); SPIKE-0001 owns the fix and must not be steering-ratified
until it is addressed.

## Acceptance criteria

- [ ] SPIKE-0001's cost model replaces the bare `K·L_p99` latency reservation with a
      term that accounts for **intra-phase max-of-M order-statistic tail**, parameterised
      by the per-phase parallel-GET count M (the frontier width). Acceptable forms:
      (a) a per-phase term `E[max of M GETs]` or its P99, summed/convolved over K phases;
      (b) an explicit, documented bound `L_phase(M) ≥ L_p99` with the multiplier derived,
      not assumed.
- [ ] The envelope is **defined jointly over (s, B_max, K, M)** — M (max parallel GETs
      per phase, equivalently frontier width) becomes a first-class envelope parameter,
      because the byte budget and the latency budget are coupled through it.
- [ ] The derivation shows a concrete feasible point that closes P99 ≤ 1 s **with the
      corrected latency term** at both 1 Gbps and the binding 50 Mbps case, and shows the
      2 s ceiling holds across the whole ratified envelope (worst in-envelope M, K).
- [ ] The discrete-event simulation (EPIC-003) models max-of-M parallel GETs per phase
      (not a single GET per phase) and reproduces the corrected analytical P99 within the
      documented tolerance.
- [ ] Out-of-envelope detection accounts for M: a plan whose frontier width pushes the
      latency term over budget is flagged out-of-envelope at plan time, even if its total
      bytes are < B_max.
- [ ] `docs/commanders-intent.md`, `master-rubric.md` Cat. 3, `core-requirements.md` R7,
      and `formal-verification-policy.md` are updated so the documented formula matches the
      ratified cost model (docs-curator / planner; keep the headline 4 MB / 75 MB figures
      but annotate that the latency reservation includes the intra-phase tail).
- [ ] Steering re-ratification: `steering-perf-sla` + `steering-formal-methods` sign off
      the corrected SPIKE-0001 before EPIC-001 / EPIC-002 dependent tasks flip to `ready`.

## Notes / log

- 2026-06-13T18:30Z `steering-formal-methods` (ratification pass): filed. Decision record:
  `.project/decisions/0001-latency-budget-intra-phase-tail.md`. This does NOT block the
  launch — it is an instruction binding SPIKE-0001's ratification. The launch is APPROVED;
  this item is the tracked obligation.
- Reproduction is in the decision record (Python, seeded). Re-run it against the calibrated
  S3 distribution once real numbers arrive.
