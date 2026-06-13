---
id: T-0021
title: Write-clause executor (CREATE/MERGE/SET/DELETE/REMOVE) + txn TCK
type: task
status: backlog
priority: P1
assignee:
epic: EPIC-002
deps: [T-0019, T-0010, BUG-0006]
rubric_refs: [4, 1]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

Phase-2 of the TCK ramp: write clauses (CREATE, MERGE, SET, DELETE, REMOVE) and
transaction scenarios. Writes stage mutations into a new version and commit via the
atomic manifest swap (T-0010). TCK write scenarios assert **side effects**
(`+nodes`, `-relationships`, ...) that need the QueryStatistics surface from
BUG-0006 / decision 0007 to be observable. Design-gated on the commit path
(T-0010, gated on SPIKE-0002). See `EPIC-002`, `EPIC-004`.

## Acceptance criteria
- [ ] Parser/planner/executor extended for CREATE, MERGE, SET, DELETE, REMOVE with correct openCypher semantics.
- [ ] Writes commit atomically via T-0010's manifest swap; a failed write leaves no partial data (ties to T-0013).
- [ ] QueryStatistics surface (BUG-0006) reports side-effect counts so the TCK `Then the side effects should be:` steps are observable and assertable.
- [ ] Phase-2 write + transaction TCK scenarios move from `pending` to `pass`.
- [ ] tests added (unit + integration on the mock + TCK delta); coverage not regressed
- [ ] docs / ADR updated if behaviour changed
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: depends on the commit path (T-0010) and BUG-0006 (stats
surface). This is the Phase-2 TCK driver.
