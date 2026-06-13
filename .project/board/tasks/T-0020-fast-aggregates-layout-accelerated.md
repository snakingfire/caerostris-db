---
id: T-0020
title: Layout-accelerated aggregates (count/sum/avg/min/max/collect/distinct)
type: task
status: backlog
priority: P2
assignee:
epic: EPIC-002
deps: [T-0019, T-0009]
rubric_refs: [6, 4]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

Cat. 6 (fast aggregates): `count`, `sum`, `avg`, `min`, `max`, `collect`,
`distinct` must exploit the layout — e.g. `count` of a label from manifest
statistics (T-0009) without scanning, `sum`/`min`/`max` from a single columnar
range scan, `distinct` from sorted runs — rather than a full-graph traversal. Must
beat a naïve full-scan on a representative dataset (benchmark). See `EPIC-002`,
`EPIC-001` (Cat. 6 is co-owned).

## Acceptance criteria
- [ ] `count(*)` / `count(label)` answered from manifest statistics where possible (no data-object scan), falling back to columnar scan when a predicate narrows the set.
- [ ] `sum`/`avg`/`min`/`max` over a node property use a single columnar range scan of that property column.
- [ ] `distinct` and `collect` produce correct openCypher semantics (null handling, ordering where specified).
- [ ] Benchmark: aggregate query measurably faster than a naïve full-scan baseline on the representative dataset (T-0009 stats vs. scan).
- [ ] Aggregate TCK scenarios move from `pending` to `pass`.
- [ ] tests added (unit + integration + bench + TCK delta); coverage not regressed
- [ ] docs / ADR updated if behaviour changed
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: depends on the executor (T-0019) and manifest statistics
(T-0009). Cat. 6 is non-gate (P2) but cheap once the read path exists.
