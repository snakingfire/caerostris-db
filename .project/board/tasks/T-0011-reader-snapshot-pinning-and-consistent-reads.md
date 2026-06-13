---
id: T-0011
title: Implement reader snapshot pinning + consistent-snapshot reads
type: task
status: readypriority: P1
assignee:
epic: EPIC-004
deps: [SPIKE-0002, T-0009]
rubric_refs: [1, 7, 11]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

A reader resolves the latest manifest at open (or txn-begin) time, pins that
version V, and serves all subsequent reads from V's objects even while a writer
commits V+1 — snapshot isolation. Pinning must also keep V's objects safe from GC
(coordinated with T-0012). Design-gated on SPIKE-0002's reader-snapshot-pinning
design. See `EPIC-004`, `EPIC-006` (concurrent readers).

## Acceptance criteria
- [ ] A reader pins manifest version V on open; all reads resolve through V's object set regardless of concurrent commits.
- [ ] Snapshot-isolation test: a reader holding V sees a stable graph while a writer commits V+1 in a separate thread/process; the reader never observes V+1 data or a torn state.
- [ ] Pinning registers the version with the GC-safety mechanism (T-0012) so pinned objects are retained.
- [ ] Read path is consistent with the SPIKE-0002 TLA+ snapshot-isolation invariant.
- [ ] tests added (unit + integration on the mock; concurrent reader/writer property test); coverage not regressed
- [ ] docs / ADR updated if behaviour changed
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: blocked on SPIKE-0002. Pairs with T-0010 (writer) and
T-0012 (GC); together they realise the Cat. 1 commit protocol.
