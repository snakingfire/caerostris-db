# Decision 0034 — BUG-0017 lands a generation-fenced `CachingObjectStore`, carrying the cache module

- **Status:** decided (implementer, autonomous)
- **Date:** T0+3:40 (2026-06-13)
- **Owner:** implementer (BUG-0017)
- **Rubric refs:** Cat. 9 (caching), Cat. 1 (ACID / no stale reads — GATE)
- **Supersedes for the cache module:** the unlanded T-0033 branch
  `work/T-0033-optional-cache-wrapper-lru-resource-aware`

## Context

BUG-0017 fixes a lost-invalidation race in `CachingObjectStore::load_object`:
a cold read fetches from the backend with **no lock held** between the fetch and
the cache populate, so a concurrent `invalidate` / `invalidate_all` (fired by a
committing writer) in that window is a no-op and the reader then writes the
**pre-commit** bytes into the cache. Subsequent reads serve the stale version
indefinitely — violating the no-stale-read invariant (Cat. 1) and T-0033 AC #4.

The bug was filed against `src/storage/cache.rs`, which lives **only** on the
unlanded T-0033 branch. On the current `main` there is **no `cache.rs`** and no
cache module reference in `src/storage/mod.rs`. The T-0033 branch is ~94 commits
behind `main` and has accumulated unrelated drift (it would delete a large amount
of already-landed work if merged as-is). The BUG-0017 board item explicitly
authorizes landing the fix "as this follow-up" rather than in T-0033.

## Decision

Implement BUG-0017 as a standalone PR **based on the latest `main`** that
introduces a *correct-by-construction*, generation-fenced `CachingObjectStore`
module. The fix (the generation fence) is the substantive change; the surrounding
LRU/resource-aware cache code is the minimal carrier required for the fix to
compile and be tested, because that code is not on `main`.

The fence works as follows:

- A monotonic `generation: AtomicU64` lives in the cache.
- A cold read **snapshots** the generation under the state lock *before* the
  backend fetch.
- Any coherence-affecting mutation — `invalidate`, `invalidate_all`, `delete`,
  and out-of-band `put` — **bumps** the generation.
- On populate, the read re-acquires the state lock and inserts the fetched bytes
  **only if** the generation is unchanged. If it changed, a commit/invalidation
  raced the fetch, so the bytes are dropped (the read still returns the bytes it
  fetched — that read is serializable before the commit — but they never poison
  the cache).

## Alternatives considered

1. **Block BUG-0017 on T-0033 landing.** Rejected: T-0033 is stale and racy; its
   review gate is open precisely because of this bug. Blocking idles the lane and
   leaves a known stale-read defect in the only cache implementation.
2. **Fix inside the T-0033 branch.** Rejected: that branch is ~94 commits behind
   `main` and a merge would revert large amounts of landed work. Re-basing it is
   out of scope for a bug fix and would re-open the whole T-0033 review.
3. **Hold the inner-store lock across the populate.** Rejected: it serializes all
   backend reads behind one mutex (kills multi-reader throughput) and still does
   not coordinate with `invalidate_all`, which takes the *state* lock, not the
   inner-store lock. The generation fence is lock-cheap and precisely targeted.

## Consequences

- `src/storage/cache.rs` + `tests/cache_integration.rs` land via BUG-0017.
- T-0033's separate cache PR should be **dropped/closed** in favor of this module
  (the integrator/planner reconciles the board). If T-0033 lands first by
  accident, this PR rebases and keeps only the fence delta.
- No new dependency is added: the concurrency proof is a **deterministic
  injected-window test** (a backend whose `get()` fires the racing
  commit+invalidate in the populate window), which the board item explicitly
  permits as an alternative to `loom`.
- ADR for the cache design is recorded as
  `docs/adr/0009-optional-resource-aware-cache.md` (the T-0033 branch used `0005`,
  which already names the pluggable-index interface on `main` — avoiding that
  collision).
