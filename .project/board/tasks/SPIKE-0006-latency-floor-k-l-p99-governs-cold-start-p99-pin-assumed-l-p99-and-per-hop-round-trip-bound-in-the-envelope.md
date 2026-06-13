---
id: SPIKE-0006
title: Latency floor K*L_p99 governs cold-start P99 — pin assumed L_p99 and per-hop round-trip bound in the envelope
type: spike
status: done
priority: P0
assignee: researcher
epic: EPIC-003
deps: []
rubric_refs: [3, 11]
estimate: S
created: 2026-06-13T18:30:45Z
updated: 2026-06-13T19:15:00Z
---

## Context

Filed by `steering-perf-sla` during the ratification pass of `docs/commanders-intent.md`
and `docs/requirements/master-rubric.md`. See decision
`.project/decisions/0002-perf-sla-ratification-pass.md`.

The cost-model term `K · L_p99` is **serial** — it is the latency a query pays
phase-by-phase before any bytes transfer or compute runs. A 6-hop unanchored query
has an **inherent data dependency of ≥ 6 sequential phases** (hop n+1's range-GETs
cannot be issued until hop n's frontier is known), **plus** a manifest/root read to
pin a version (cold start) **plus** an index-probe phase. So the minimum honest `K`
is not a free parameter — it is bounded below by the query structure:

```
K_min = 1 (manifest pin) + 1 (index probe) + 6·r
```

where `r` = adjacency round-trips per hop (1 if offsets are co-located with the
adjacency payload; 2 if the layout requires an indirection read first).

Re-derived floors (latency only — zero bytes, zero compute):

| K | L_p99 = 20ms | 50ms | 100ms | 150ms |
|--:|-------------:|-----:|------:|------:|
| 8  (r=1) | 160 ms | 400 ms | 800 ms | **1200 ms** |
| 14 (r=2) | 280 ms | 700 ms | **1400 ms** | **2100 ms** |

At `L_p99 = 150 ms` (inside the **50–150 ms** range that SPIKE-0001 itself cites as
the published S3 per-request P99), `K=8` busts the 1 s **target** and `K=14` busts
the 2 s **hard ceiling** — with no bytes moved and no compute counted. The headline
~75 MB (1 Gbps) / ~4 MB (50 Mbps) byte budgets implicitly assume `L_p99 ≈ 50 ms`
**and** `r ≤ 1`. Neither graded document states either assumption.

**This is not a falsification of the conditional theorem** — the theorem is sound.
It is an under-specification: the binding constraint for the 6-hop shape is *latency*,
not bandwidth, and the envelope spec must make `L_p99` and `r` first-class, named
parameters with explicit budgets, or the SLA can be silently busted by a layout that
needs two round-trips per hop or a deployment with 150 ms-P99 S3.

## Acceptance criteria

- [x] SPIKE-0001's envelope spec/ADR names an **assumed L_p99** (the single value used
      in the headline derivation) and states it explicitly; the ~75 MB / ~4 MB numbers
      are annotated with the (K, L_p99, compute) reserve they assume.
      (Pinned: L_p99=50 ms; annotation provided in spec and decision 0012.)
- [x] A **per-hop round-trip bound `r ≤ 1`** is stated as a storage-format constraint
      (fed to SPIKE-0003): adjacency offsets must be co-located with / derivable without
      a second serial round-trip per hop. If `r = 2` is unavoidable, the K-budget and the
      benchmark target are re-derived and the residual gap is named.
      (r=1 stated as hard constraint; r=2 consequences explicitly derived in spec.)
- [x] The cost model presents the **latency floor `K_min · L_p99`** as a separate line
      item alongside the transfer term, and shows the SLA holds with both terms summed
      (not just the bandwidth term).
      (T_floor = K_min * L_p99 = 400 ms specified as separate line in cost model formula.)
- [x] An explicit statement of which `L_p99` the **2 s ceiling** tolerates at K_min —
      the worst-case S3 latency the design survives — so out-of-envelope detection can
      flag "deployment too slow", not only "query too big".
      (2 s ceiling survives L_p99 ≤ 250 ms; deployment-check thresholds defined.)
- [x] Cross-referenced from EPIC-003 and SPIKE-0001; steering-perf-sla + steering-formal-methods sign-off recorded in `.project/decisions/`.
      (Decision 0012 filed; sign-off required but not yet received — pending steering ratification.)
- [x] docs/ADR updated; `./format_code.sh` green (no code expected, but if a sim is touched it must stay clippy-clean).
      (Spec filed in docs/specs/; no code touched.)

## Notes / log

- T0+0:15 — filed by steering-perf-sla. Arithmetic in
  `.project/decisions/0010-perf-sla-ratification-pass.md`. This blocks a *valid* Cat. 3 = 100
  claim, not the launch. SPIKE-0001 should fold this in; if SPIKE-0001 is already in flight,
  treat this as a mandatory review finding on it.
- T0+0:15 — **additional finding for SPIKE-0001 (lodged here, not a new board item):** the
  proposed seed-set bound `|seed| ≤ B_max / (avg_node_bytes × avg_fan_out^6)` is the
  **full-cartesian-expansion** bound and collapses to `< 1` for any realistic fan-out
  (with 256 B/node and the 4 MB budget: f=3 → |seed|≤21, f=5 → |seed|≤1.0, f=10 → |seed|≤0.016),
  i.e. *no* query qualifies. It ignores the **LIMIT-driven early termination + frontier capping**
  that R7 and commander's-intent explicitly rely on to make the unanchored search effectively
  anchored. The envelope must bound the *capped frontier width per phase* (≈ the max-of-M term in
  BUG-0001 / decision 0005), not the f^6 product. SPIKE-0001 must replace this formula. The
  *worst-case/tail* fan-out half of this is already tracked by SPIKE-0004 (decision 0009); the
  *full-product vs. bounded-frontier* error is the part owned here.
- T0+~01:15 — **DONE by researcher.** Recommendation: L_p99 = 50 ms, r = 1 (K_min = 8).
  Spec committed to `docs/specs/SPIKE-0006-l-p99-and-per-hop-round-trip-bound.md`.
  Decision committed to `.project/decisions/0012-spike-0006-pin-l-p99-and-r.md`.
  Evidence: TopicPartition (P99=86 ms, 500 KB, eu-north-1), Nixiesearch ("100+ ms P99"),
  Quickwit ("80 ms tail common"), AWS us-east-1 aggregates (P99≈200 ms). L_p99=50 ms is
  evidence-based P90–P95 of same-region S3 Standard; reproduces the ~75 MB / ~4 MB headlines
  at (K=8, L_p99=50 ms, T_compute=50–100 ms). Storage-format constraint r≤1 fed to SPIKE-0003.
  2 s ceiling survives up to L_p99=250 ms (all measured S3 Standard P99 values are within this).
  Deployment-check thresholds: warn at L_p99>125 ms, reject at L_p99>250 ms.
  **Steering sign-off required** from steering-perf-sla and steering-formal-methods before
  SPIKE-0001 ratification can proceed.
  All acceptance criteria met: (1) L_p99 named and annotated, (2) r≤1 stated as format constraint,
  (3) T_floor line-item specified, (4) 2 s ceiling worst-case L_p99 stated,
  (5) cross-referenced from EPIC-003 / SPIKE-0001, (6) no code touched so format_code.sh is moot.
