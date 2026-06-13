# ADR 0009 — Optional, resource-aware caching wrapper with a generation fence

## Status

`accepted`

## Date / T+ marker

T0+3:45 (2026-06-13)

## Context

Cat. 9 of the master rubric requires an **optional, resource-aware local cache**
that speeds warm queries while the cold-start SLA holds with the cache **off**
(the cache is never a crutch — commander's intent). The cache wraps the
`ObjectStore` abstraction (`src/storage/mod.rs`) so disabling it is a single
config flag and requires no engine-code changes.

A first implementation existed on the unlanded T-0033 branch but carried a
**lost-invalidation race** (BUG-0017): a cold read fetched bytes from the backend
holding no lock, and then populated the cache in a separate step. A committing
writer's `invalidate` / `invalidate_all` firing in that window was a no-op (the
entry was not yet present), and the reader then cached the **pre-commit** bytes,
serving a stale version indefinitely. That violates the no-stale-read invariant
(Cat. 1, GATE). The bug code never reached `main`; this ADR documents the cache
as it lands on `main` via BUG-0017 (see `.project/decisions/0034-*`).

## Decision

We will ship `CachingObjectStore`, a wrapper over `Arc<Mutex<dyn ObjectStore>>`,
with:

- **Optionality:** `CacheConfig::enabled == false` (the default) makes the
  wrapper a pure pass-through that allocates and caches nothing.
- **Resource-awareness:** a bounded two-tier (memory + optional disk) LRU.
  Per-entry cost is `key.len() + value.len()`; an object larger than its tier's
  budget is never cached (served straight from the backend), so the cache can
  never exceed its budget or OOM.
- **A generation fence for cold-read coherence (the BUG-0017 fix):** a monotonic
  `AtomicU64` generation counter. A cold read snapshots the generation under the
  state lock *before* the backend fetch; every coherence-affecting mutation
  (`invalidate`, `invalidate_all`, `delete`, out-of-band `put`) bumps it; the
  populate re-acquires the state lock and inserts the fetched bytes **only if the
  generation is unchanged**. If a commit raced the fetch, the bytes are dropped
  rather than poisoning the cache. The racing read still returns the bytes it
  fetched (a read serializable *before* the commit), but the cache is never left
  holding a superseded version.

## Consequences

- **No stale reads after a commit**, even under the Arc-shared multi-reader
  topology, proven by a deterministic injected-window integration test
  (`tests/cache_integration.rs`) — no new dependency (`loom`) is required.
- **No serialization penalty:** the backend fetch is still done with no lock
  held, so concurrent cold readers do not block one another. The fence is two
  cheap atomic loads plus a counter bump on mutation.
- **Write-through `put`** is unconditional (the wrapper produced the bytes, so
  there is no race for its own write) but still bumps the generation to fence out
  any in-flight populate of the same key.
- This ADR uses number `0009`; the T-0033 branch had used `0005`, which already
  names the pluggable-index interface on `main` (collision avoided).
