---
id: STORY-001
title: TCK Phase-3 breadth — drive pass-rate to 100%
type: story
status: backlog
priority: P1
assignee:
epic: EPIC-002
deps: [T-0019, T-0021]
rubric_refs: [4]
estimate: L
created: T0+0:20
updated: T0+0:20
---

## Context

The long tail of Cat. 4 (the only category whose 100% is required for "done"):
once read (T-0019) and write (T-0021) phases pass, this story tracks the remaining
TCK breadth — full expression/function library, list/map operations, path
functions, comprehensions, complex pattern semantics — to 100% pass with no skipped
scenarios. Per decision 0008, `pending` counts in the denominator; "100%" means
every scenario for the pinned tag passes. This is an `L` umbrella; child tasks are
filed per failing scenario bucket as the pass-rate climbs. See `EPIC-002`.

## Acceptance criteria
- [ ] Failing/pending TCK scenario buckets enumerated from the live T-0002 report; each bucket gets a child task as it is tackled.
- [ ] openCypher expression + function library (string/numeric/list/map/temporal-stub, predicates, comprehensions) implemented to cover the remaining scenarios.
- [ ] TCK pass-rate (pending-in-denominator, pinned tag) reaches 100% in CI with zero skipped scenarios.
- [ ] tests: TCK pass-rate delta is the metric; per-bucket unit tests added
- [ ] docs updated as language coverage decisions are made
- [ ] `./format_code.sh` green

## Notes / log
This is an `L` umbrella story (not directly assignable) — split into per-bucket
`S`/`M` child tasks against the live TCK report. Parallelise the tail across many
implementer + test-author agents per the behind-pace doctrine.
