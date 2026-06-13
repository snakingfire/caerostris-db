# ADR 0009 — Generation fence for the optional cache's cold-read populate (BUG-0017)

## Status

`accepted`

## Date / T+ marker

T0+4:05 (2026-06-13)

## Context

Cat. 9 of the master rubric requires an **optional, resource-aware local cache**
that speeds warm queries while the cold-start SLA holds with the cache **off**
(the cache is never a crutch — commander's intent). T-0033 landed `CachingStore`
on `main` (`src/storage/cache.rs`): an `ObjectStore` wrapper with a byte-bounded
LRU in-memory tier, off by default.

T-0033 landed with a **known, documented** lost-invalidation race (BUG-0017), and
its own module docs explicitly forbade enabling the cache under a concurrent
invalidator and made BUG-0017 a hard dependency of the cache-wiring task
(T-0040). The race: `CachingStore::get` on a miss fetches bytes from the backend
holding **no** cache-state lock (intentionally, so concurrent cold readers are
not serialised), then re-acquires the cache lock to populate. A committing
writer's `invalidate` / `invalidate_all` (or the `invalidate` issued by a
concurrent `put`/`delete`) firing in that window hits an empty slot and is a
no-op; the reader then caches the **pre-commit** bytes and serves the stale
version indefinitely — a snapshot-isolation violation (Cat. 1, GATE).

## Decision

We will close the window with a **monotonic generation counter held inside the
cache state** (`LruByteCache`), so it moves under the very `Mutex` that guards
the map:

- A cold read snapshots the generation under the cache lock *before* releasing it
  for the backend fetch.
- `invalidate` and `invalidate_all` — and therefore the `invalidate` that
  `put`/`delete` issue — bump the generation while holding the cache lock.
- The populate re-acquires the cache lock and inserts the fetched bytes **only if
  the generation is unchanged** (`LruByteCache::insert_if_current`). If a commit
  raced the fetch, the bytes are dropped rather than caching a superseded version.

The racing read still returns the bytes it fetched (a read serializable *before*
the commit is correct), but the cache is never left holding a stale version.
Eviction (`evict_one`) deliberately does **not** bump the generation: reclaiming
space for an unrelated key is not an invalidation and must not fence out an
in-flight populate.

## Consequences

- **No stale reads after a commit**, even under the Arc-shared single-writer /
  multi-reader topology — proven by a deterministic injected-window integration
  test (`tests/cache_integration.rs`) that fires the racing commit+invalidate in
  the populate window. No new dependency (`loom`) is required.
- **No serialization penalty:** the backend fetch is still done with no cache
  lock held, so concurrent cold readers do not block one another. The fence adds
  one `u64` read on the miss path and one increment on each invalidation, both
  under a lock the code already takes.
- **Unblocks T-0040:** the cache may now be wired into the engine in an enabled,
  Arc-shared multi-reader configuration. The module's BUG-0017 warning is
  replaced with documentation of the fence.
- This ADR is numbered `0009`; `0005` already names the pluggable-index interface
  on `main`.
