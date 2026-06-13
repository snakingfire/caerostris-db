---
id: T-0013
title: Crash / partial-write recovery property tests at every commit phase
type: task
status: backlog
priority: P1
assignee:
epic: EPIC-004
deps: [T-0010, T-0011]
rubric_refs: [1, 10, 11]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

Cat. 1 = 100 requires that a simulated failure at each commit phase (mid-write,
mid-swap, post-swap-pre-ack) leaves the database in the pre-commit state with no
partial data visible to readers. This task injects failures at each modelled phase
and asserts recovery, exercising the durability-barrier and atomicity invariants
from SPIKE-0002 / SPIKE-0005. See `EPIC-004`, `EPIC-009`.

## Acceptance criteria
- [ ] A fault-injection harness can abort a commit at each phase boundary (post-stage / pre-swap / post-swap-pre-ack).
- [ ] After each injected failure, a fresh reader sees the pre-commit version V (never partial V+1) — asserted per phase.
- [ ] Property test: arbitrary write sequences with random abort points always yield a consistent, readable snapshot.
- [ ] Recovery does not require manual intervention; reopening the DB resolves the correct version automatically.
- [ ] tests added (integration + property on the mock); coverage not regressed
- [ ] docs / ADR updated if a recovery gap is found (file a BUG if so)
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: depends on the commit/read implementation (T-0010, T-0011),
which are themselves gated on SPIKE-0002. This is the crash-recovery evidence for
the Cat. 1 GATE.
