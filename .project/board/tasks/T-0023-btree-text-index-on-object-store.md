---
id: T-0023
title: B-tree index on text node properties, persisted on object storage
type: task
status: backlog
priority: P2
assignee:
epic: EPIC-005
deps: [T-0022, SPIKE-0003, T-0010]
rubric_refs: [5, 3]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

The first concrete index: a B-tree on text node properties, stored on object
storage in a format compatible with the EPIC-001 layout (SPIKE-0003) and committed
atomically alongside data via the manifest swap (T-0010). Cold reads must resolve a
leaf in one or two range GETs so index access does not blow the latency budget.
Design-gated on SPIKE-0003 (object layout) + the commit path (T-0010). See
`EPIC-005`, `EPIC-001`.

## Acceptance criteria
- [ ] B-tree supports equality and prefix lookup on a text property; nodes laid out on object storage as range-GET-friendly objects per SPIKE-0003.
- [ ] Index is updated transactionally with data: one commit = data + index update via T-0010; a crashed commit never leaves index and data out of sync (tested).
- [ ] A leaf lookup for a cold query resolves in ≤ 2 range GETs (asserted on the mock; supports the latency budget).
- [ ] Implements the T-0022 trait without leaking B-tree internals to the planner API.
- [ ] tests added (unit on B-tree ops + integration on the mock); coverage not regressed
- [ ] docs / ADR updated if behaviour changed
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: blocked on SPIKE-0003 + T-0010. Coordinate index-object naming
with the storage-format spec so index objects fit the layout naturally.
