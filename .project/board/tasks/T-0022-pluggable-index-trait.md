---
id: T-0022
title: Define pluggable secondary-index trait (insert/delete/lookup/range)
type: task
status: ready
priority: P2
assignee:
epic: EPIC-005
deps: [T-0006]
rubric_refs: [5]
estimate: S
created: T0+0:20
updated: T0+0:20
---

## Context

Cat. 5 = 100 requires a pluggable index trait so B-tree, range, full-text, and
spatial indices can be added without rewriting core planner/storage logic. The
trait can be designed now against the data model (T-0006); concrete implementations
that persist to object storage come later (gated on SPIKE-0003). Designing the
interface first prevents B-tree specifics from leaking. See `EPIC-005`.

## Acceptance criteria
- [ ] `SecondaryIndex` trait defined: `insert`, `delete`, point `lookup`, and `range_scan`; associated key/value types parameterised so non-B-tree indices fit.
- [ ] A planner-facing query API consults the trait by selectivity without knowing the concrete index type.
- [ ] An in-memory reference implementation of the trait exists for unit-testing the interface (not the object-store B-tree yet).
- [ ] The trait carries no B-tree-specific assumptions (verified by sketching a second index type's signature against it).
- [ ] tests added (unit on the in-memory impl); coverage not regressed
- [ ] docs / ADR updated with the index-interface decision
- [ ] `./format_code.sh` green

## Notes / log
Ready now: depends only on the data model (T-0006). The object-store B-tree
(T-0023) and the extensibility stub (T-0025) build on this trait.
