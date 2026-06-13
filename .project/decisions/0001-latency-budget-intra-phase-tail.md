# Decision 0001 — Latency budget must account for the intra-phase max-of-M tail

- **Date:** 2026-06-13 (T0+~00:06)
- **Author / role:** `steering-formal-methods`
- **Type:** ratification-pass finding (design-level; binds SPIKE-0001)
- **Status:** decided — tracked as `BUG-0001` (P0); launch APPROVED
- **Rubric:** Cat. 3 (latency envelope, GATE, w14), Cat. 11 (formal artifacts, GATE, w6)
- **Affects:** `docs/commanders-intent.md`, `docs/requirements/master-rubric.md`,
  `docs/requirements/core-requirements.md` (R7), `docs/process/formal-verification-policy.md`,
  `SPIKE-0001`, `EPIC-003`

## Context

Ratification pass over the commander's intent and master rubric. My hardest mandate: if no
set of parameters lets in-envelope queries fit in 1 s, the design is falsified — escalate.
So I verified the latency theorem's arithmetic from first principles rather than trusting
the asserted headline numbers.

## What I checked

**1. Does the byte budget close at all?** Formula as documented:
`B_max = W × (T_budget − K·L_p99 − T_compute)`, T_budget = 1 s.

Sweeping L_p99 ∈ {50,100,150} ms, K ∈ {3,5,8,10}, compute ∈ {50,100,200} ms confirms a
feasible region exists. The intent's headline figures are reproduced exactly:
- **1 Gbps ⇒ ~75 MB** at L_p99=100 ms, K=3, compute=100 ms (usable 600 ms → 75.0 MB).
- **50 Mbps ⇒ ~4 MB** at L_p99=50 ms, K=3, compute=200 ms (usable 650 ms → 4.06 MB);
  also L_p99=100 ms/K=3/compute=50 ms → 4.06 MB.

**Verdict on feasibility: the theorem is NOT falsified.** A clear feasible parameter region
exists and the 50 Mbps binding case closes. Good — the design's central invariant survives.

**2. Is the latency term itself sound?** This is the catch. The formula reserves `K·L_p99`,
treating each of the K sequential phases as completing at the *per-request* P99. But the
design mandates "few, large, **parallel** range GETs" over an expanding frontier. A phase
issuing M parallel GETs completes at the **max of M** GET latencies (an order statistic),
whose tail is worse than a single GET's. Meanwhile summing K phases *concentrates* the tail
(CLT). Which effect dominates decides whether the formula is conservative or optimistic.

Monte-Carlo probe (lognormal GET, P50=20 ms, P99=100 ms; query latency = Σ over K phases of
max-of-M GETs; 50 000 trials; seed=1):

| K | M   | query P99 | naive K·L_p99 | ratio |
|--:|----:|----------:|--------------:|------:|
| 3 | 1   | 193 ms    | 300 ms        | 0.64  |
| 3 | 8   | 332 ms    | 300 ms        | 1.11  |
| 3 | 64  | 527 ms    | 300 ms        | 1.76  |
| 3 | 256 | 693 ms    | 300 ms        | 2.31  |
| 5 | 1   | 272 ms    | 500 ms        | 0.54  |
| 5 | 8   | 491 ms    | 500 ms        | 0.98  |
| 5 | 64  | 790 ms    | 500 ms        | 1.58  |
| 5 | 256 | 1034 ms   | 500 ms        | 2.07  |

At M=1 the formula is conservative (ratio < 1). At the realistic high-fan-out regime
(M = tens–hundreds of parallel range GETs per frontier phase) it is **optimistic by up to
~2.3×**, and at K=5/M=256 the latency term alone (1034 ms) already exceeds the 1 s target
**before counting one transferred byte**.

## Decision

1. **APPROVE the launch.** The latency theorem closes; this is an under-specification, not a
   falsification. Per ratification doctrine we surface-and-track rather than hard-block.

2. **Bind SPIKE-0001 (via `BUG-0001`, P0):** the cost model must replace the bare `K·L_p99`
   latency reservation with a term that accounts for the intra-phase max-of-M order statistic,
   and the **envelope must become jointly defined over (s, B_max, K, M)** — frontier width M
   is a first-class envelope parameter because byte budget and latency budget are coupled
   through it. The simulation must model max-of-M per phase. Out-of-envelope detection must
   flag plans whose frontier width busts the latency term even when total bytes < B_max.
   SPIKE-0001 must NOT be steering-ratified until this is addressed.

3. **Docs reconciliation:** keep the 4 MB / 75 MB headline figures (they are correct for the
   feasible point) but annotate the formula in intent/rubric/R7/policy so the documented
   reservation matches the ratified model. Owner: docs-curator / planner, sequenced with
   SPIKE-0001 ratification.

## Alternatives considered

- **Do nothing / accept the formula as-is.** Rejected: it lets the analytical model
  under-reserve latency, so the sim and the real benchmark could miss the SLA with no anchor
  in the spec explaining why — the "fast only with luck" trap the intent explicitly forbids.
- **Drop M and just shrink K.** Rejected: K and M are independent levers; a 6-hop search with
  a tiny seed set can still fan a wide frontier in a single phase. The budget must constrain M
  explicitly, not hope K covers it.
- **Hard-block the launch and escalate to the full committee.** Rejected: the theorem closes,
  so this does not meet the escalation bar ("no parameters fit"). Tracking via P0 is correct.

## Secondary finding (non-blocking, tracked here, not its own board item)

Formal-artifact **paths are inconsistent across canonical docs**, which can cause the
rubric-grader to score Cat. 11 as "missing" when an artifact exists at the other path:
- `formal-verification-policy.md` and the `steering-formal-methods` agent def: TLA+/sim under
  `formal/` (`formal/commit-protocol/`, `formal/latency-model/`, `formal/latency-sim/`).
- Board items SPIKE-0002 / EPIC-004: TLA+ under `docs/formal/`.
- ADRs referenced as both `docs/adr/` (the directory that actually exists, with a template)
  and `docs/adrs/` (SPIKE-0001/0002/0003).

Recommendation: canonicalise on `formal/` for model/sim/spec artifacts and `docs/adr/` for
ADRs (these match the policy and the existing directory), and fix the board items + grader
inputs to match. Owner: planner / docs-curator. Folded into `BUG-0001`'s docs-reconciliation
criterion to avoid a redundant board item.

## Reproduction

Both checks are deterministic Python (seed=1), runnable standalone; the byte-budget sweep and
the Monte-Carlo probe are reproduced verbatim in this record's tables. Re-run the probe against
the calibrated S3 latency distribution when real credentials arrive (per the policy's
calibration step).
