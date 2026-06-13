# Decision 0034 — BUG-0017 fixes the landed `CachingStore` with a generation fence

- **Status:** decided (implementer, autonomous)
- **Date:** T0+4:05 (2026-06-13)
- **Owner:** implementer (BUG-0017)
- **Rubric refs:** Cat. 9 (caching), Cat. 1 (ACID / no stale reads — GATE)

## Context

BUG-0017 fixes a lost-invalidation race in the optional cache. T-0033 (the cache
wrapper) **landed on `main`** as `CachingStore` (`src/storage/cache.rs`) with the
race present and **explicitly documented** in its module header: it forbade
enabling the cache under a concurrent invalidator and named BUG-0017 as a hard
dependency of the cache-wiring task (T-0040).

The race: `CachingStore::get` on a miss fetches from the backend holding no
cache-state lock, then re-acquires the lock to populate. A committing writer's
`invalidate` / `invalidate_all` (or the `invalidate` from a concurrent
`put`/`delete`) firing in that window hits an empty slot (no-op); the reader then
caches the **pre-commit** bytes and serves the stale version indefinitely — a
no-stale-read / snapshot-isolation violation (Cat. 1, GATE) and T-0033 AC #4.

> **Note on path:** this work was first drafted against a stale, unlanded T-0033
> branch (`CachingObjectStore`, a two-tier API that never reached `main`). Once
> `main` advanced and the real T-0033 (`CachingStore`) was observed to have
> landed, the branch was rebased onto the latest `main` and the fix re-authored
> against the **actual** landed code. No stale-branch code is carried.

## Decision

Apply a **monotonic generation counter inside `LruByteCache`** (guarded by the
same `Mutex` as the map):

- The cold read snapshots the generation under the cache lock before the backend
  fetch.
- `invalidate` / `invalidate_all` (hence `put`/`delete`) bump it under the lock.
- The populate inserts only if the generation is unchanged
  (`insert_if_current`); otherwise it drops the fetched bytes.
- Eviction does **not** bump the generation (space reclamation is not an
  invalidation).

## Alternatives considered

1. **Block BUG-0017 / leave the cache disabled-only.** Rejected: the cache is on
   `main` with a known stale-read defect; leaving it unfixed blocks T-0040 and
   leaves a Cat. 1 hazard one config flag away.
2. **Hold the inner-store lock across the populate.** Rejected: it serialises all
   backend reads behind one mutex (kills multi-reader throughput) and still does
   not coordinate with `invalidate_all`, which takes the *cache* lock.
3. **Version-key every cache entry by manifest generation.** Heavier: requires
   the cache to know the manifest version on every read. The counter-fence is
   local to the wrapper and dependency-free.

## Consequences

- The change is a small delta to the landed `CachingStore` plus three new
  integration tests (the two race scenarios + an unraced-miss control). No new
  dependency: the race is reproduced with a deterministic injected-window test.
- **Unblocks T-0040** (engine cache wiring). The module's BUG-0017 warning is
  replaced with documentation of the fence; ADR `0009` records the design.
