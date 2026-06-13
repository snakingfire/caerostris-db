---
id: BUG-0017
title: CachingObjectStore lost-invalidation race serves stale reads after commit (no generation fence on populate)
type: bug
status: in_review
priority: P1
assignee: implementer-wf_fe688db0-093-29
epic: EPIC-008
deps: [T-0033]
rubric_refs: [9, 1]
estimate: S
created: T0+3:20
updated: T0+4:10
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

T0+4:10 — implementer-wf_fe688db0-093-29: claimed and implemented on
`work/BUG-0017-...`, rebased onto the **latest `main`**. T-0033 has since
**landed** (`CachingStore`, `src/storage/cache.rs`) with this race present and
explicitly documented; the fix is a small **delta to the landed `CachingStore`**
(an earlier draft against the stale, never-landed `CachingObjectStore` T-0033
branch was discarded on rebase — see Decision 0034). Fix = monotonic generation
counter inside `LruByteCache`: the cold read snapshots the generation under the
cache lock before the backend fetch; `invalidate`/`invalidate_all` (hence
`put`/`delete`) bump it; populate inserts only if the generation is unchanged
(`insert_if_current`), else drops the bytes; eviction does not bump it.
Reproduced the race deterministically (injected-window, no loom): RED with the
racy `insert` (2 race tests fail, stale v1/old served), GREEN with the fence.
Full suite 311 passed; cache: 20 unit + 9 integration green; `./format_code.sh`
exit 0. cache.rs module docs updated (BUG-0017 warning replaced with the fence
docs); ADR-0009 + Decision 0034 recorded; T-0040 cross-ref updated. PR.md
filled; status -> in_review; review gate (adversarial-reviewer +
premortem-analyst) pending dispatch.
