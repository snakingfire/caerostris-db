---
id: T-0037
title: Ratchet coverage threshold to 90% + property-based ACID invariant suite
type: task
status: backlog
priority: P1
assignee:
epic: EPIC-009
deps: [T-0005, T-0010, T-0011, T-0019]
rubric_refs: [10, 1]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

Cat. 10 GATE requires ≥90% line coverage in CI and property-based tests covering
ACID invariants (arbitrary write sequences → consistent reads). T-0005 stands up
cargo-llvm-cov at a 0% floor; this task ratchets the threshold to 90% once enough
of the engine exists and adds the proptest ACID suite. Depends on the coverage
infra (T-0005) and the commit/read/executor paths being present to cover. See
`EPIC-009`, `EPIC-004`.

## Acceptance criteria
- [ ] cargo-llvm-cov CI threshold ratcheted to 90%; the build fails below it.
- [ ] Reported line coverage ≥ 90% across the engine (gaps named if any remain, with a plan).
- [ ] Property-based suite (proptest): arbitrary interleaved write sequences always produce a consistent, readable snapshot (atomicity + isolation), shrinking on failure.
- [ ] The property suite runs in CI within a bounded time budget.
- [ ] tests added (proptest); coverage not regressed
- [ ] docs updated noting the 90% ratchet
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: depends on T-0005 (coverage infra) and the commit/read/executor
tasks producing coverable code. Ratchet late so the empty-crate build is never blocked.
