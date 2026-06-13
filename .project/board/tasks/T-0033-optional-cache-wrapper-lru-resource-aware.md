---
id: T-0033
title: Optional resource-aware cache wrapper around ObjectStore (LRU, bounded)
type: task
status: readypriority: P2
assignee:
epic: EPIC-008
deps: [T-0001]
rubric_refs: [9]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

Cat. 9 (caching): an optional cache wrapping the `ObjectStore` abstraction (T-0001).
It must be architecturally optional — disabling it is a single config flag, not a
refactor — and resource-aware (size-bounded, LRU eviction, no OOM). The cold-start
SLA must hold with the cache off; that test is T-0034. The wrapper itself only
needs the object-store trait, so it is ready now. See `EPIC-008`.

## Acceptance criteria
- [ ] Cache implemented as a wrapper around `Arc<dyn ObjectStore>`; disabling it is a single config flag with no engine code changes.
- [ ] Configurable: max memory budget (bytes), optional disk-cache path + size, eviction policy (LRU minimum), on/off toggle.
- [ ] Resource-aware: cache never exceeds its configured budget; evicts under pressure rather than OOM-ing (tested with a tight budget).
- [ ] Correctness: version-keyed (or invalidated on commit) so a reader never sees stale data after a commit invalidates a cached object — tested.
- [ ] Warm-query micro-benchmark: a repeated read is measurably faster with cache on than off.
- [ ] tests added (unit + integration on the mock); coverage not regressed
- [ ] docs / ADR updated with the cache config interface
- [ ] `./format_code.sh` green

## Notes / log
Ready now: depends only on T-0001's object-store trait. The cold-SLA-without-cache
proof is T-0034 (depends on the benchmark T-0016).
