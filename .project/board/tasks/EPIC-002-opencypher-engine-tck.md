---
id: EPIC-002
title: openCypher engine — 100% TCK pass-rate
type: epic
status: backlog
priority: P0
assignee:
epic:
deps: []
rubric_refs: [4, 6]
estimate: L
created: T0
updated: T0
---

## Context

openCypher compliance (Cat. 4, weight 12, GATE) is one of the two highest-weight requirements alongside ACID correctness. The acceptance bar is 100% of the official openCypher Technology Compatibility Kit (TCK) — a phased ramp is allowed (P1: reads; P2: writes + transactions; P3: full breadth), but a permanent curated subset is not. The live TCK pass-rate is the metric the rubric grader reads from CI.

This epic covers the full openCypher engine stack: lexer, parser, AST, planner, executor, and runtime. The planner must exploit the storage layout from EPIC-001 (range reads, secondary indices from EPIC-005) to push filtering down and minimise bytes transferred. Fast aggregates (Cat. 6, weight 5) — `count`, `sum`, `distinct`, and all openCypher aggregation functions — must exploit the columnar layout (e.g. pre-computed counts from metadata, columnar scan for `sum`, sorted runs for `distinct`) rather than degrade to full-graph traversal. The aggregate implementation is co-owned by this epic and EPIC-001.

The TCK harness (T-0002) must be wired before language work starts, so the pass-rate is continuously observable. Agents throw parallel coding + testing effort at the long tail to converge to 100%.

Relevant requirements: R1 (graph data model), R10 (100% openCypher), R6 (fast aggregates), R7 (latency — planner must bound query scope).

## Acceptance criteria

- [ ] TCK harness wired (see T-0002) and running in CI; pass-rate emitted as a machine-readable number the rubric grader consumes.
- [ ] Phase 1 milestone: all read-only TCK scenarios (MATCH, WHERE, RETURN, WITH, UNWIND, ORDER BY, LIMIT, SKIP, basic pattern matching) passing.
- [ ] Phase 2 milestone: write TCK scenarios passing (CREATE, MERGE, SET, DELETE, REMOVE) and transaction scenarios passing.
- [ ] Phase 3 milestone: full TCK breadth — 100% pass-rate in CI, no skipped scenarios.
- [ ] Planner performs filter push-down and uses secondary indices (from EPIC-005) when available to anchor unanchored matches.
- [ ] Fast aggregates: `count`, `sum`, `avg`, `min`, `max`, `collect`, `distinct` exploit layout metadata or columnar scan; benchmark demonstrates improvement over naïve full-scan on a representative dataset.
- [ ] All openCypher data types and property types handled correctly (strings, integers, floats, booleans, lists, maps, null).
- [ ] `./format_code.sh` green; no clippy warnings.

## Notes / log

T-0002 (TCK harness wire-up) is the immediate prerequisite and is `ready` from T0. Planner design and executor architecture should be specced (ADR committed) before major implementation begins to avoid costly rework.
