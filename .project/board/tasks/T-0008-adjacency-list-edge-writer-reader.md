---
id: T-0008
title: Implement compressed adjacency-list edge writer + chunked range reader
type: task
status: backlog
priority: P1
assignee:
epic: EPIC-001
deps: [SPIKE-0003, T-0006]
rubric_refs: [2, 3]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

The edge side of the layout: edges stored as compressed adjacency lists sorted by
source node id, chunked so a hop expansion fetches only the frontier's adjacency
without reading the whole edge set. Per `SPIKE-0008` falsification F1, adjacency
chunking must allow **early-abort partial reads** so the binding 50 Mbps / 4 MB
case is feasible. Design-gated on SPIKE-0003. See `EPIC-001` and the storage-format
spec owned by `SPIKE-0003` (lands under `docs/adr/`).

## Acceptance criteria
- [ ] Writer serialises edges into source-sorted, chunked adjacency-list objects per the SPIKE-0003 spec; chunk granularity matches the spec's range-read access pattern.
- [ ] Reader expands the out-neighbours of a given node id via a bounded range GET; supports early-abort (stop fetching once the LIMIT-driven frontier is satisfied) per SPIKE-0008 F1.
- [ ] Round-trip fidelity: arbitrary directed typed edge sets (proptest) serialise and read back identical, including edge properties.
- [ ] A single-hop expansion for an in-envelope frontier reads ≤ its allotted B_max share (per SPIKE-0001) — asserted on the mock.
- [ ] tests added (unit + integration on the S3 mock); coverage not regressed
- [ ] docs / ADR updated if format detail deviates from SPIKE-0003
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: blocked on SPIKE-0003. Coordinate chunk sizing with the
cost-model bytes budget from SPIKE-0001 / BUG-0004.
