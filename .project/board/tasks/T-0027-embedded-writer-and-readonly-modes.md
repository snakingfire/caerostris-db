---
id: T-0027
title: Embedded writer-master + embedded read-only attach modes
type: task
status: backlog
priority: P1
assignee:
epic: EPIC-006
deps: [T-0026, T-0011]
rubric_refs: [7]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

Two of the four attach modes: embedded writer-master (opens DB, acquires the
writer lease, reads + writes) and embedded read-only (attaches to a DB whose
writer-master is a separate process; reads consistent snapshots). The open-time API
must clearly express the chosen mode and make misuse (write in read-only) an early
error. Depends on the lease (T-0026) and snapshot reads (T-0011). See `EPIC-006`.

## Acceptance criteria
- [ ] Embedded writer-master: opens DB, acquires lease, reads + writes; tested end-to-end on the mock.
- [ ] Embedded read-only: attaches to a DB with a separate writer-master process; reads see consistent snapshots while the writer commits; tested with a concurrent writer.
- [ ] Open-time API expresses the mode; attempting a write in read-only mode is a compile-time or early-runtime error (tested).
- [ ] tests added (unit + integration on the mock); coverage not regressed
- [ ] docs / ADR updated with the attach-mode API
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: depends on T-0026 (lease) + T-0011 (snapshot reads). Two of the
four Cat. 7 modes; master-less + server in T-0028 / T-0029.
