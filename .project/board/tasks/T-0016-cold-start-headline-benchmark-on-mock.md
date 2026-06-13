---
id: T-0016
title: Cold-start headline 6-hop benchmark on injected-latency mock (cache off)
type: task
status: backlog
priority: P1
assignee:
epic: EPIC-003
deps: [SPIKE-0007, T-0007, T-0008, T-0015]
rubric_refs: [3, 10]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

Cat. 3 = 100 requires a benchmark on the local S3 mock with injected P99 latency
that demonstrates the cold-start target met for a representative in-envelope query,
**with cache disabled**. The measurement protocol must follow `SPIKE-0007` /
decision 0010: no criterion warm-up, a fresh version pin each iteration, a named
injected-latency profile, cache off. Design-gated on SPIKE-0007 (measurement
protocol) and needs the storage readers (T-0007/T-0008) and detection (T-0015).
See `EPIC-003`, `docs/process/testing-and-benchmarks.md`.

## Acceptance criteria
- [ ] A criterion (or custom) benchmark runs the headline 6-hop unanchored, node-property-filtered, LIMIT 10 query against the mock with a named injected-latency profile.
- [ ] Measurement follows SPIKE-0007: no warm-up reuse of cached objects, fresh snapshot per iteration, cache explicitly disabled.
- [ ] Reported P99 ≤ 1 s (target) / ≤ 2 s (hard ceiling) for the in-envelope query; result recorded as a baseline artifact.
- [ ] The measured P99 is consistent with the T-0014 simulation prediction (within the stated tolerance).
- [ ] tests/benches added; baseline committed under `benches/baselines/`; coverage not regressed
- [ ] docs updated with the benchmark methodology
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: blocked on SPIKE-0007 + the storage readers + the planner
detection. This is the measured-SLA evidence for the Cat. 3 GATE.
