---
id: BUG-0017
title: CachingObjectStore lost-invalidation race serves stale reads after commit (no generation fence on populate)
type: bug
status: in_progress
priority: P1
assignee: implementer-wf_fe688db0-093-29
epic: EPIC-008
deps: [T-0033]
rubric_refs: [9, 1]
estimate: S
created: T0+3:20
updated: T0+3:40
---

## Context

Found during adversarial review of T-0033
(`work/T-0033-optional-cache-wrapper-lru-resource-aware`,
`src/storage/cache.rs`).

`CachingObjectStore::load_object` fetches a cold object from the backend and then
populates the cache in two **separately-locked** steps with **no lock held in
between**:

```text
// load_object (cache.rs ~L402-406)
self.misses.fetch_add(1, ...);
let bytes = self.inner.lock()...get(key)?;   // inner-store MutexGuard drops at ';'
self.cache_object(key, &bytes);              // re-acquires the state lock
```

Between the backend `get` returning and `cache_object` storing the bytes, the
wrapper holds neither the inner-store lock nor the cache-state lock. If a
concurrent writer commits a new version and the commit path calls
`invalidate(key)` / `invalidate_all()` in that window, the invalidation is a
**no-op** (the entry is not yet in the cache), and the reader then writes the
**pre-commit** bytes into the cache. Every subsequent read serves the stale
version indefinitely.

This violates T-0033 acceptance criterion 4 ("version-keyed (or invalidated on
commit) so a reader never sees stale data after a commit invalidates a cached
object") and the snapshot-isolation / no-stale-read invariant (Cat. 1). The
multi-reader model (concurrent readers share one `CachingObjectStore` via `Arc`)
makes the interleaving real, not theoretical — `invalidate_all()` does not fix it
either, since the same populate-after-invalidate window applies to any object.

## Reproduction

Confirmed **deterministically** during review with a backend whose `get()`
models a commit firing in the post-fetch / pre-populate window: a cold read of
`manifest` returns `v1`, the backend advances to `v2` and `invalidate("manifest")`
runs (cache empty → no-op), the reader populates `v1`, and the next read returns
the stale `v1` (expected `v2`). Probe was run in the worktree and removed (not
committed).

## Acceptance criteria
- [ ] Add a monotonic generation/version counter to the cache state; snapshot it
      under the state lock before the backend fetch.
- [ ] On populate, re-acquire the state lock and insert only if the generation is
      unchanged (i.e. no `invalidate`/`invalidate_all`/`put`/`delete` for that key
      occurred during the fetch); otherwise drop the fetched bytes.
- [ ] `invalidate`, `invalidate_all`, `delete`, and out-of-band `put` bump the
      generation so an in-flight populate is fenced out.
- [ ] Concurrent test: reader miss-populate interleaved with commit+invalidate
      never yields a stale read (loom or a deterministic injected-window test).
- [ ] tests added; coverage not regressed; `./format_code.sh` green.

## Notes / log
Filed by adversarial-reviewer during the T-0033 review gate. T-0033 receives
`changes_requested` on this finding; fix can land in T-0033 itself or as this
follow-up — but T-0033 must not land while criterion 4 is unmet.
