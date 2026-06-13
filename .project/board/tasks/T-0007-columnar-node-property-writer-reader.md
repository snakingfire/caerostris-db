---
id: T-0007
title: Implement columnar node-property object writer + range-read reader
type: task
status: readypriority: P1
assignee:
epic: EPIC-001
deps: [SPIKE-0003, T-0006]
rubric_refs: [2, 3]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

The node side of the on-object layout: nodes stored columnar (one column per
property, sorted by node id for range access) so the planner can issue a single
large range GET over a contiguous node-id span. This is the storage-format
implementation and is **design-gated on SPIKE-0003** (the format spec), which in
turn depends on SPIKE-0001's byte budget. Stays `backlog` until SPIKE-0003 is
steering-ratified (`steering-storage`). See `EPIC-001` and the storage-format spec
owned by `SPIKE-0003` (lands under `docs/adr/`).

## Acceptance criteria
- [ ] Writer serialises a batch of nodes into columnar node-property objects per the SPIKE-0003 spec (field order, encoding, alignment, object naming).
- [ ] Reader reconstructs nodes from those objects; a `get_range` over an id span fetches only the relevant partition (verified: bytes fetched ≤ partition size, not whole-object).
- [ ] Round-trip fidelity: arbitrary node sets (proptest) serialise and read back identical (labels + all property types).
- [ ] A representative range-GET for an in-envelope node span reads ≤ B_max share allotted to the node side (per SPIKE-0001 budget) — asserted in an integration test on the mock.
- [ ] tests added (unit + integration on the S3 mock); coverage not regressed
- [ ] docs / ADR updated if the format detail deviates from SPIKE-0003
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: blocked on SPIKE-0003 (storage format spec) ratification.
Flip to `ready` only when SPIKE-0003 is `done`.
