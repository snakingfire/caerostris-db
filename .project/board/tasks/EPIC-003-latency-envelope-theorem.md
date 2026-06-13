---
id: EPIC-003
title: Latency selectivity-envelope theorem + cold-start SLA
type: epic
status: backlog
priority: P0
assignee:
epic:
deps: []
rubric_refs: [3, 11]
estimate: L
created: T0
updated: T0
---

## Context

The headline workload — **6-hop unanchored MATCH with node-property filter, LIMIT 10, over 1B nodes / 10B edges, cold start, P99 ≤ 1 s** (2 s hard ceiling) at the client on a 1 Gbps link — is achievable only **conditionally**. The physics are decisive: at 50 Mbps a 1 s budget allows only ~4 MB of S3 reads; at 1 Gbps ~75 MB. An unconstrained degree-10 6-hop expansion touches 10⁶+ paths = hundreds of MB to GB. No storage layout makes that fit.

This epic formalises the conditional theorem (Cat. 3, weight 14, GATE) and produces the evidence the rubric grader requires. The deliverables are: (1) the **selectivity envelope** — a precise definition of selectivity `s`, byte budget `B_max = bandwidth × (latency_budget − K·L_p99 − compute)`, and round-trip phase bound `K`; (2) an **analytical cost model** proving in-envelope queries hit the SLA; (3) a **discrete-event simulation** calibrated to real S3 latency distributions that corroborates the model; (4) **out-of-envelope detection** in the planner (reject/warn/degrade — never silent SLA miss); and (5) a **benchmark** on the mock with injected latency that meets the SLA without the local cache.

This is design-before-code: SPIKE-0001 is the first concrete deliverable and must be steering-ratified (steering-perf-sla + steering-formal-methods) before any storage or query-execution tasks become `ready`. The formal artifacts overlap with Cat. 11 (formal verification) — the cost model and simulation double as formal evidence for that category.

Relevant requirements: R7 (selectivity-envelope theorem), R9 (cache must not be required), R11 (formal verification), R12 (benchmarks).

## Acceptance criteria

- [ ] Envelope precisely defined and committed as a spec/ADR: selectivity bound `s`, byte budget `B_max` derived for 1 Gbps (≈75 MB) and 50 Mbps (≈4 MB), phase bound `K`, maximum seed-set size and per-hop fan-out bound.
- [ ] Analytical cost model committed: proves that any query satisfying the envelope constraints hits P99 ≤ 1 s cold, showing the algebra step by step.
- [ ] Discrete-event simulation committed (Rust or Python): calibrated to realistic S3 latency distributions; simulation outputs match the cost model predictions.
- [ ] Out-of-envelope detection implemented in the planner: estimated bytes-read or fan-out exceeding B_max causes an explicit error/warning at plan time, never a silent SLA miss.
- [ ] Benchmark on the local S3 mock with injected P99 latency demonstrates the cold-start target met for a representative in-envelope query, **with cache disabled**.
- [ ] Steering-ratification record committed (steering-perf-sla + steering-formal-methods sign-off) before implementation tasks in EPIC-001 / EPIC-002 that depend on this go `ready`.
- [ ] `./format_code.sh` green; no clippy warnings in simulation code.

## Notes / log

SPIKE-0001 is the first task; its output is the steering-ratified model doc/ADR. All storage and query execution tasks that must serve the envelope carry SPIKE-0001 in their deps.
