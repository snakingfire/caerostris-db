---
id: T-0019
title: Implement read-query executor over the storage readers
type: task
status: backlog
priority: P1
assignee:
epic: EPIC-002
deps: [T-0018, T-0007, T-0008, T-0011]
rubric_refs: [4, 3]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

The executor runs a logical plan against the columnar node reader (T-0007), the
adjacency-list reader (T-0008), and a pinned snapshot (T-0011), producing result
rows. It implements the Phase-1 read operators (scan, expand, filter, project,
order, skip, limit) and drives the TCK read scenarios toward passing. Depends on
the storage readers, which are design-gated on SPIKE-0003, so this stays `backlog`
until those land. See `EPIC-002`.

## Acceptance criteria
- [ ] Executor runs MATCH/WHERE/RETURN/WITH/UNWIND/ORDER BY/SKIP/LIMIT plans against the storage readers over a pinned snapshot, returning correct rows.
- [ ] Bounded frontier expansion + LIMIT-driven early termination so an in-envelope query stops fetching once LIMIT is satisfied (ties into T-0008 early-abort).
- [ ] Hop expansion uses the adjacency reader's range GETs (no full edge scan for a seeded match).
- [ ] Phase-1 read TCK scenarios that the parser/planner support move from `pending` to `pass` (measurable pass-rate increase via T-0002).
- [ ] tests added (unit + integration on the mock + TCK delta); coverage not regressed
- [ ] docs / ADR updated if behaviour changed
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: depends on storage readers (T-0007/T-0008, gated on SPIKE-0003)
and snapshot pinning (T-0011, gated on SPIKE-0002). This is the Phase-1 TCK driver.
