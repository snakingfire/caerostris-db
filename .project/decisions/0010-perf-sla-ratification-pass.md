# Decision 0010 — Perf-SLA ratification pass: latency floor (serial K depth) + benchmark-validity confound

- **Date:** 2026-06-13 (T0+~00:15)
- **Author / role:** `steering-perf-sla`
- **Type:** ratification-pass findings (design-level; bind SPIKE-0001 and the Cat. 3 benchmark)
- **Status:** decided — tracked as `SPIKE-0006` (P0) + `SPIKE-0007` (P0); launch **APPROVED**
- **Rubric:** Cat. 3 (latency envelope, GATE, w14), Cat. 11 (formal artifacts, GATE, w6),
  Cat. 10 (tests/benches, GATE, w8)
- **Affects:** `docs/commanders-intent.md`, `docs/requirements/master-rubric.md`,
  `docs/requirements/core-requirements.md` (R7), `docs/process/testing-and-benchmarks.md`,
  `SPIKE-0001`, `EPIC-003`
- **Complements:** decision `0005-latency-budget-intra-phase-tail.md` (steering-formal-methods,
  BUG-0001). That finding addresses the *within-phase width* of the latency term (max-of-M order
  statistic). This decision addresses the *serial depth* of the latency term and the *measurement
  method*. The two are orthogonal and both bind the same cost model. Also complements decision
  `0009-planner-stats-and-tail-fanout-bound.md` (SPIKE-0004): that pins the *estimator inputs*
  (tail/worst-case fan-out from maintained manifest statistics); this pins the *cost-model algebra*
  the estimator feeds. Decision numbers churned during the parallel ratification pass — refer to
  filenames/owners, not the leading integer.

## Mandate

If no set of envelope parameters lets in-envelope queries fit in 1 s, the design is falsified —
escalate. I re-derived the arithmetic from first principles rather than trusting the headline.

## What I checked

### 1. Headline byte budgets close (no falsification)

`B_max = bandwidth × (T_budget − K·L_p99 − T_compute)`, T_budget = 1 s.

A consistent reserve of ~400 ms (e.g. K=6, L_p99=50 ms, compute=100 ms) reproduces both
headline figures from a *single* reserve:

| reserve | usable | 1 Gbps | 50 Mbps |
|--------:|-------:|-------:|--------:|
| 300 ms | 700 ms | 87.5 MB | 4.38 MB |
| 360 ms | 640 ms | 80.0 MB | 4.00 MB |
| 400 ms | 600 ms | **75.0 MB** | **3.75 MB** |

The intent's "~75 MB / ~4 MB" are correct order-of-magnitude. **Feasibility: NOT falsified.**

### 2. The serial latency floor K_min·L_p99 is under-specified (→ SPIKE-0006, P0)

`K·L_p99` is a **serial** term. A 6-hop unanchored query has an inherent **≥ 6 sequential
dependency phases** (hop n+1 cannot issue until hop n's frontier is known), plus a manifest/root
pin (cold start) and an index probe:

```
K_min = 1 (manifest) + 1 (index) + 6·r,   r = adjacency round-trips per hop
```

Latency-only floors (zero bytes, zero compute):

| K | L_p99=20ms | 50ms | 100ms | 150ms |
|--:|-----------:|-----:|------:|------:|
| 8  (r=1) | 160 | 400 | 800 | **1200** ms |
| 14 (r=2) | 280 | 700 | **1400** | **2100** ms |

At L_p99 = 150 ms (the top of SPIKE-0001's own cited 50–150 ms S3 range), K=8 busts the 1 s
**target** and K=14 busts the 2 s **ceiling** — before a byte moves. The headline budgets
silently assume **L_p99 ≈ 50 ms AND r ≤ 1**. Neither graded doc states either. The binding
constraint for the 6-hop shape is *latency depth*, not bandwidth.

### 3. The benchmark measurement is under-specified and has a warm-up confound (→ SPIKE-0007, P0)

The graded docs require "cold-start ... without the cache" but define no valid cold-start
measurement. The only protocol lives in `testing-and-benchmarks.md`, which describes criterion
with a **"default: 10-sample warm-up"** — the opposite of a cold start. If the grader accepts a
standard `cargo bench` P99 as Cat. 3 evidence, it scores a warm-process / warm-page-cache /
warm-version-pin number against a cold SLA — the exact "fast only when warm" falsification the
intent forbids, slipping in through the *measurement* rather than the design. Separately, the
SLA "P99 ≤ 1 s on the mock" names no **injected-latency profile**; a green obtained under
`loopback` (0 ms) or `fast-s3` (5 ms) is not evidence the real-S3 SLA holds.

## Decision

1. **APPROVE the launch.** The latency theorem closes; these are under-specifications, not
   falsifications. Per ratification doctrine: surface-and-track, do not hard-block.

2. **Bind SPIKE-0001 (via SPIKE-0006, P0):** the envelope spec must name an assumed `L_p99`,
   present `K_min·L_p99` as an explicit line item, impose a storage-format constraint `r ≤ 1`
   (fed to SPIKE-0003), and state the worst-case `L_p99` the 2 s ceiling survives so
   out-of-envelope detection can also flag "deployment too slow". SPIKE-0001 must NOT be
   ratified (perf-sla half) until SPIKE-0006 **and** BUG-0001 (the max-of-M term) are folded in.
   Combined, K_min sets the serial depth and M sets the per-phase width; the cost model needs
   both to be honest.

3. **Bind the Cat. 3 benchmark (via SPIKE-0007, P0):** define a cold-start measurement protocol
   (fresh state per sample / no criterion warm-up, cache explicitly OFF with a CI-enforced test,
   a named injected-latency profile with a pinned acceptance bar — recommend target = nominal-s3
   20 ms, ceiling = slow-s3 50 ms, to be ratified — and a stated N + P99 estimator). Update the
   grader's Cat. 3 evidence rule so a warm/loopback number is not accepted as cold-start evidence.

4. **Docs reconciliation:** keep the 75 MB / 4 MB headlines; annotate the formula in
   intent/rubric/R7 with the (K, L_p99, r, compute) reserve once SPIKE-0001 ratifies. Sequence
   with decision 0001's docs-reconciliation so the two are folded together. Owner: docs-curator /
   planner.

## Secondary finding (non-blocking, tracked here)

**Bandwidth-case ambiguity in the graded bar.** The rubric "Targets" and intent Mission state the
acceptance bar as 1 Gbps; R7 line 74 makes 50 Mbps "*ideally also* tolerable". My steering
mandate treats 50 Mbps as the binding constraint and "1 Gbps-only" as incomplete. So the grader
could score Cat. 3 = 100 on a 1 Gbps-only result while steering would withhold the perf-sla
sign-off. Recommendation: the rubric should state that the **analytical/sim proof must cover both
1 Gbps and 50 Mbps** (the 50 Mbps case being where the byte budget is tightest), while the
**measured mock benchmark** acceptance bar remains the 1 Gbps-equivalent profile. This keeps the
proof honest about the binding case without requiring a 50 Mbps physical link in CI. Owner:
planner / steering-perf-sla, fold into SPIKE-0006.

## Alternatives considered

- **Accept the formula/measurement as-is.** Rejected: lets the model under-reserve serial latency
  and lets a warm benchmark masquerade as cold — both are the "fast only when warm/lucky" trap.
- **Hard-block and escalate to the full committee.** Rejected: the theorem closes (a feasible
  parameter region exists), so the escalation bar ("no parameters fit") is not met. P0 tracking is
  the correct response.
- **Fold into BUG-0001 rather than new items.** Rejected: BUG-0001 is the width/M term; the serial
  K-depth and the benchmark-validity confound are distinct work with different owners (SPIKE-0006:
  perf-sla + formal-methods; SPIKE-0007: perf-engineer + grader). Separate items keep the board honest.

## Reproduction

Deterministic arithmetic (Python, no RNG): the B_max reserve table, the K_min·L_p99 floor table,
and the seed-set/fan-out collapse check are reproduced in `SPIKE-0006` and re-runnable standalone.
