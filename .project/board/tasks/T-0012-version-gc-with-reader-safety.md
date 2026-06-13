---
id: T-0012
title: Implement safe version GC (retention grace / pinned-version protection)
type: task
status: backlog
priority: P2
assignee:
epic: EPIC-004
deps: [SPIKE-0002, SPIKE-0003, T-0010, T-0011]
rubric_refs: [1, 2]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

Old manifest versions and their now-unreferenced data objects must be reclaimed
without deleting anything a snapshot-pinned reader still needs. Per `SPIKE-0008`
F3, the policy is a retention grace window / TTL'd pins so GC never races a live
reader. Orphaned pre-swap objects from a crashed commit (SPIKE-0005 Constraint 3)
must be GC-able and never referenced. Design-gated on SPIKE-0002 + SPIKE-0003.
See `EPIC-004`, `EPIC-001`.

## Acceptance criteria
- [ ] GC identifies objects unreferenced by any retained manifest and deletes them per the retention-grace policy from SPIKE-0008 F3.
- [ ] Reader-safety test: GC running concurrently with a reader pinned to version V never deletes an object V references; the reader completes successfully.
- [ ] Orphan cleanup: data objects from a simulated crashed pre-swap commit are detected as unreferenced and collected; they were never visible to readers.
- [ ] GC is non-blocking with respect to readers and the writer (does not hold a lock that stalls commits).
- [ ] tests added (unit + integration on the mock); coverage not regressed
- [ ] docs / ADR updated if behaviour changed
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: blocked on SPIKE-0002, SPIKE-0003, and the commit/read tasks
(T-0010, T-0011). Lower priority than the write/read core but required for Cat. 1 = 100.
