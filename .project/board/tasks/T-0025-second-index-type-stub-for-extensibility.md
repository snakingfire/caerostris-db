---
id: T-0025
title: Stub a second index type against the trait to prove extensibility
type: task
status: ready
priority: P3
assignee:
epic: EPIC-005
deps: [T-0022]
rubric_refs: [5]
estimate: S
created: T0+0:20
updated: T0+0:20
---

## Context

Cat. 5 = 100 requires a second index type stubbed against the same trait to prove
the interface is not leaking B-tree specifics. A range index (or a stub full-text
index) implemented against T-0022's trait demonstrates extensibility without a core
rewrite. Can be done as soon as the trait (T-0022) exists. See `EPIC-005`.

## Acceptance criteria
- [ ] A second index type (range index or stub full-text) implements the T-0022 trait.
- [ ] The implementation required no change to the trait signature (proves the interface generalises) — documented.
- [ ] The planner can consult it through the same trait-based API as the B-tree.
- [ ] tests added (unit covering the second index's trait conformance); coverage not regressed
- [ ] docs / ADR updated noting the extensibility demonstration
- [ ] `./format_code.sh` green

## Notes / log
Ready once T-0022 lands. P3 (extensibility proof) — pull after the B-tree path if
agents are free; cheap and de-risks the Cat. 5 = 100 anchor.
