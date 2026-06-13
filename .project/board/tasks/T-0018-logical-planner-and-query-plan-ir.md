---
id: T-0018
title: Logical query planner + plan IR with filter push-down
type: task
status: in_progress
priority: P1
assignee: implementer-T0018
epic: EPIC-002
deps: [T-0017]
rubric_refs: [4, 3]
estimate: M
created: T0+0:20
updated: T0+3:22
---

## Context

Translate the AST into a logical query-plan IR (scans, expands, filters, projects,
aggregates, limit) with filter push-down so node-property predicates anchor the
match as early as possible. The plan IR is the surface the out-of-envelope estimator
(T-0015) and the index selection (EPIC-005) operate on, and the executor (T-0019)
consumes. The IR can be designed now against the data model (T-0006); the estimator
and index hooks attach later. See `EPIC-002`, `EPIC-003`.

## Acceptance criteria
- [ ] Plan IR defined: operators for node scan, expand (hop), filter, project, aggregate, order, skip/limit; documented.
- [ ] AST → plan IR lowering for read queries; filter push-down moves WHERE predicates to the earliest operator that can evaluate them.
- [ ] Plan IR exposes the estimates the out-of-envelope detector (T-0015) needs (per-operator cardinality/byte hooks), without yet wiring the detector.
- [ ] Plan is inspectable (a debug/`EXPLAIN`-style dump) so tests can assert push-down and operator order.
- [ ] tests added (unit on representative plans, asserting push-down); coverage not regressed
- [ ] docs / ADR updated if planner architecture decided
- [ ] `./format_code.sh` green

## Notes / log
Ready now: depends only on the parser (T-0017). Detector wiring (T-0015) and index
selection (EPIC-005) plug into this IR once their designs are ratified.
