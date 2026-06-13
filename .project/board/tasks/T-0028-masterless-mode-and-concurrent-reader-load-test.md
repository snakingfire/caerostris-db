---
id: T-0028
title: Embedded master-less mode + concurrent-reader load test
type: task
status: backlog
priority: P1
assignee:
epic: EPIC-006
deps: [T-0027]
rubric_refs: [7]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

The third attach mode plus the concurrency-under-load evidence for Cat. 7 = 100:
embedded master-less mode opens a DB with no live writer and reads the latest
committed manifest; and a load test with N ≥ 4 parallel readers against a
writer-master shows no corrupted or inconsistent results. Depends on the embedded
modes (T-0027). See `EPIC-006`.

## Acceptance criteria
- [ ] Embedded master-less mode: opens a DB with no live writer; reads succeed against the latest committed manifest; tested.
- [ ] Concurrent-reader load test: N ≥ 4 parallel readers query a writer-master under active commits; every reader sees a valid, consistent snapshot; no corruption — asserted.
- [ ] No reader interferes with the writer or other readers (throughput sanity recorded).
- [ ] tests added (integration + load test on the mock); coverage not regressed
- [ ] docs / ADR updated if behaviour changed
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: depends on T-0027. Provides the third mode + the
concurrent-reader-under-load evidence the Cat. 7 GATE requires.
