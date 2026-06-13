---
id: T-0040
title: Cache configuration surface + opt-in engine wiring (default off)
type: task
status: backlog
priority: P3
assignee:
epic: EPIC-008
deps: [T-0033, T-0019, BUG-0017]
rubric_refs: [9]
estimate: S
created: T0+0:20
updated: T0+3:35
---

## Context

Wire the cache wrapper (T-0033) into the engine read path as an **opt-in** layer
that defaults to off, with a clean configuration surface (open-time option / config
struct). The engine must never assume the cache is present — disabling it stays a
single flag. This is what makes the Cat. 9 "architecturally optional" claim true in
the real read path rather than just in the wrapper. See `EPIC-008`, `EPIC-002`.

## Acceptance criteria
- [ ] Cache config exposed at open/attach time (max memory, disk path/size, eviction, on/off); default is off.
- [ ] The engine read path (T-0019) routes object reads through the cache only when enabled; with it off, the path is byte-for-byte the no-cache path.
- [ ] A test confirms that toggling the cache flag requires no other code/config change (single-flag claim).
- [ ] With cache on, a repeated query is measurably faster; with cache off, behaviour and results are identical.
- [ ] tests added (unit + integration on the mock); coverage not regressed
- [ ] docs updated with the config surface
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: depends on the cache wrapper (T-0033) and the read path
(T-0019). P3 — the cold-SLA-off guard (T-0034) is the higher-value Cat. 9 evidence.

- T+3:35 — BUG-0017 (lost-invalidation race in CachingStore populate window) added
  as a hard dep. An enabled CachingStore must NOT be wired into the read path until
  BUG-0017's generation-fence fix lands; see cache.rs Concurrency warning section.
  See acceptance criteria: add a criterion confirming enabled-cache wiring only ships
  after BUG-0017 is resolved (i.e. the fence is in place and the concurrent test
  passes).
- T+4:05 — BUG-0017's generation fence is implemented and in review
  (`work/BUG-0017-...`). `cache.rs` now documents the fence (the old "Concurrency
  warning" section is replaced); the deterministic race tests pass. Once BUG-0017
  lands, this dep clears and T-0040 may wire an enabled cache. See ADR-0009 /
  Decision 0034.
