---
id: T-0018
title: Logical query planner + plan IR with filter push-down
type: task
status: done
priority: P1
assignee: implementer-T0018
epic: EPIC-002
deps: [T-0017]
rubric_refs: [4, 3]
estimate: M
created: T0+0:20
updated: T0+4:20
landed_sha: e924d77
---

## Context

Translate the AST into a logical query-plan IR (scans, expands, filters, projects,
aggregates, limit) with filter push-down so node-property predicates anchor the
match as early as possible. The plan IR is the surface the out-of-envelope estimator
(T-0015) and the index selection (EPIC-005) operate on, and the executor (T-0019)
consumes. The IR can be designed now against the data model (T-0006); the estimator
and index hooks attach later. See `EPIC-002`, `EPIC-003`.

## Acceptance criteria
- [x] Plan IR defined: operators for node scan, expand (hop), filter, project, aggregate, order, skip/limit (+ optional, unwind, empty); documented.
- [x] AST → plan IR lowering for read queries; filter push-down moves WHERE predicates to the earliest operator that can evaluate them.
- [x] Plan IR exposes the estimates the out-of-envelope detector (T-0015) needs (per-operator cardinality/byte/tail-fan-out hooks via `Estimates`), without yet wiring the detector.
- [x] Plan is inspectable (`LogicalPlan::explain()` EXPLAIN-style dump); tests assert push-down and operator order against it.
- [x] tests added (37 planner unit tests + 1 doctest, asserting push-down both directions); coverage not regressed (densely tested module)
- [x] docs / ADR updated if planner architecture decided (module-level rationale; no new ADR — follows ratified ADR-0001 + decision 0009)
- [x] `./format_code.sh` green

## Notes / log
Ready now: depends only on the parser (T-0017). Detector wiring (T-0015) and index
selection (EPIC-005) plug into this IR once their designs are ratified.

- T0+4:20 (implementer-T0018): LANDED on main, merge commit `e924d77` (--no-ff of
  work/T-0018-logical-planner; planner commits dfc6992 + 28ce6f0 + 30a4626). New
  `src/planner/`: Plan IR (`plan.rs`), AST lowering with filter push-down
  (`lower.rs` + `pushdown.rs`), `PlanError` (`error.rs`). Operators landed:
  NodeScan, LabelScan, Expand, Filter, Project, Aggregate, Sort, Skip, Limit,
  Optional, Unwind, Empty. Filter push-down anchors single-var node-property
  predicates directly on the scan (ADR-0001 §2/§2.3 selectivity anchor) and
  rests cross-var predicates above the binding expand; conjunctive WHERE split
  on AND; destination-node labels lowered to a `__has_labels` filter; named-edge
  property maps lowered to filters. `Estimates` hooks (tail fan-out, never mean;
  decision 0009) shaped for the T-0015 OOE detector, stamped `unknown()`
  (conservative). Cleared adversarial-reviewer + premortem-analyst gate (both
  approve, Round 2) — the Round-1 silent-mis-plan holes (dropped destination
  labels, anon-node self-loops, silently-truncated multi-pattern MATCH, collapsed
  var-length, dropped edge props) were reworked into faithful-lower or explicit
  `PlanError::Unsupported`; anon-node counter made per-`plan()` deterministic.
  Post-merge `cargo build` + planner tests green on main. Follow-ups for T-0015
  (wire OOE detector to `Estimates`), and lowering multi-pattern joins +
  var-length expansion (currently `Unsupported`) when their designs are ready.
