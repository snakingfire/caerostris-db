---
id: EPIC-008
title: Resource-aware optional caching
type: epic
status: backlog
priority: P2
assignee:
epic:
deps: []
rubric_refs: [9]
estimate: M
created: T0
updated: T0
---

## Context

A local cache (memory and/or disk) may speed warm queries, but it must never be a crutch: the cold-start SLA (R7, P99 ≤ 1 s) must hold with the cache **disabled** (Cat. 9, weight 4). The cache is an optional performance accelerator that respects a configurable resource budget and does not OOM the host.

This epic delivers: (1) a configurable caching layer that the storage abstraction from EPIC-001 can route object reads through; (2) resource-aware eviction (size-bounded, with an LRU or similar policy); (3) a configuration interface (max memory, max disk, on/off toggle); and (4) a test that disables the cache and confirms the cold-start SLA is still met (per the benchmark from EPIC-003).

The cache must be architecturally **optional**: disabling it is a single configuration flag, not a refactor. The rest of the engine must not assume the cache is present.

Relevant requirements: R9 (caching), R7 (cold SLA holds without cache).

## Acceptance criteria

- [ ] Cache layer implemented as an optional wrapper around the object-store abstraction; disabling it via config requires no code changes.
- [ ] Configurable parameters: maximum memory budget (bytes), optional disk-cache path and size, eviction policy (LRU at minimum), on/off toggle.
- [ ] Resource-aware: cache does not exceed its configured budget; under memory pressure it evicts rather than OOM-ing.
- [ ] Warm-query benchmark: with cache enabled, a repeated query measurably faster (wall-clock) than the cold run.
- [ ] Cold-SLA test: with cache explicitly disabled, the benchmark from EPIC-003 (injected-latency mock) still meets P99 ≤ 1 s (or the analytically-derived cold bound) for an in-envelope query.
- [ ] Cache correctness: a stale-read test confirms a reader with a cached version V object sees V+1 after a commit invalidates V (or the cache is version-keyed and never serves stale data).
- [ ] `./format_code.sh` green; no clippy warnings.

## Notes / log

P2 — delivered after the cold-SLA path (EPIC-003) and storage format (EPIC-001) are solid. The cache must be layered on top of the object-store abstraction from T-0001, not interleaved into it.
