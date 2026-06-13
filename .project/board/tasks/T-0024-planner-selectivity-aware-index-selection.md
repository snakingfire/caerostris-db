---
id: T-0024
title: Planner selectivity-aware index selection (anchor unanchored matches)
type: task
status: backlog
priority: P2
assignee:
epic: EPIC-005
deps: [T-0018, T-0023, SPIKE-0004]
rubric_refs: [5, 3, 4]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

The planner must consult available indices by selectivity and use the B-tree when a
WHERE clause filters on an indexed text property — this is what anchors an
"unanchored" 6-hop match to a tiny seed set and makes the latency envelope
practically useful. Selectivity comes from the manifest statistics (SPIKE-0004).
Falls back to scan when an index is not selective enough. See `EPIC-005`, `EPIC-003`.

## Acceptance criteria
- [ ] For `MATCH (n) WHERE n.name = 'X' ...`, the plan (`EXPLAIN` dump) shows an index lookup, not a full scan, when the B-tree index exists.
- [ ] Selectivity-driven choice: the planner uses the index when estimated selectivity (from SPIKE-0004 stats) makes it cheaper, falls back to scan otherwise — both branches tested.
- [ ] Index-anchored plans reduce estimated bytes-read below B_max for a representative in-envelope query (ties to T-0015 estimator).
- [ ] Integration test: an index-assisted query against the mock returns correct results with measurably fewer bytes read than a full scan.
- [ ] tests added (unit on plan selection + integration on the mock); coverage not regressed
- [ ] docs / ADR updated if behaviour changed
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: depends on the planner IR (T-0018), the B-tree (T-0023), and
the statistics contract (SPIKE-0004). This is what makes the envelope usable.
