---
id: T-0034
title: Test that cold-start SLA holds with the cache explicitly disabled
type: task
status: backlog
priority: P2
assignee:
epic: EPIC-008
deps: [T-0033, T-0016]
rubric_refs: [9, 3]
estimate: S
created: T0+0:20
updated: T0+0:20
---

## Context

The non-negotiable invariant: the cold-start SLA must hold **without** the cache —
the cache is never a crutch (commander's intent L40/L101). This task runs the
headline benchmark (T-0016) with the cache explicitly disabled and asserts the
target is still met, providing the Cat. 9 = 100 evidence and guarding the invariant.
See `EPIC-008`, `EPIC-003`.

## Acceptance criteria
- [ ] The T-0016 headline benchmark is run with the cache config disabled; P99 ≤ 1 s (target) / ≤ 2 s (ceiling) for the in-envelope query — asserted, not hoped.
- [ ] A guard test fails loudly if the engine path ever requires the cache to meet the SLA (e.g. by detecting cache reads on a cold run).
- [ ] The cache-on vs cache-off comparison is recorded (warm faster, cold meets SLA either way).
- [ ] tests/benches added; coverage not regressed
- [ ] docs updated linking the invariant to this test
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: depends on the cache wrapper (T-0033) and the benchmark
(T-0016). This is the evidence that the cache is optional, not load-bearing.
