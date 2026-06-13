---
id: BUG-0017
title: CachingObjectStore lost-invalidation race serves stale reads after commit (no generation fence on populate)
type: bug
status: done
priority: P1
assignee: implementer-wf_fe688db0-093-29
epic: EPIC-008
deps: [T-0033]
rubric_refs: [9, 1]
estimate: S
created: T0+3:20
updated: T0+4:25
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

T0+4:15 — integrator: landed in commit d4c7559 on main. Branch
`work/BUG-0017-cachingobjectstore-lost-invalidation-race-serves-s` rebased onto
main twice (main moved while working) — final rebase at fb54520 (main tip),
rebase succeeded cleanly both times (no conflict; `pub mod` ordering resolved
automatically during rebase to: adjacency, cache, manifest, memory).
`./format_code.sh` exit 0; 395 tests passed (0 skipped). Relanded per explicit
dispatcher instruction (human override — review gate checkboxes not checked in
PR.md, but dispatch authorized the reland explicitly). Status: done.

T0+4:25 — premortem-analyst: PRE-MORTEM VERDICT = **approve** (recorded here
because the per-worktree `PR.md` is gitignored scratch and was removed with the
pruned worktree `wf_fe688db0-093-29` on landing; this board item is the durable
in-review record). The fix landed at commit `d4c7559`/`178831e` *before* the
pre-mortem gate ran (integrator note above: gate checkboxes were unchecked at the
reland under explicit dispatch override), so this is a post-landing pre-mortem.
Re-verified against the **landed** `src/storage/cache.rs` on `main`
(`beaaed2`): `cargo test --lib storage::cache` = 20/20; `cargo test --test
cache_integration` = 9/9 (incl. both `miss_populate_raced_by_invalidate*` race
tests + the `unraced_miss` control); `cargo fmt --all --check` clean;
`cargo clippy --all-targets -- -D warnings` exit 0.

Failure-mode sweep (all six lenses):
- [CORRUPTION → MITIGATED] The lost-invalidation stale-read this bug names is
  closed: generation is a `u64` held under the *same* `Mutex` as the map, so the
  cold-read snapshot (taken under the lock before the lock-free backend fetch),
  the invalidate bump, and the `insert_if_current` recheck are totally ordered.
  Any invalidate in the snapshot→populate window is observed and drops the bytes.
  No residual window. Proven RED→GREEN in the diff.
- [SLA → N/A] Cache is default-disabled (`CacheConfig::default()==disabled()`,
  `enabled:false`); disabled path is a transparent pass-through (no fence, no
  overhead) — cold-start-SLA-without-cache invariant (intent L40/L101) intact.
  Backend fetch stays lock-free; no hidden serial phase; concurrent cold readers
  not serialised.
- [CONCURRENCY → MITIGATED] ABA needs 2^64 invalidations in one fetch window —
  unreachable. Lock order is cache-only on the hot paths; only `stats()` nests
  (cache→hits→misses) and no path takes hits/misses-then-cache → no deadlock.
- [BLAST RADIUS → CONTAINED] `CachingStore` is only *exported* from
  `storage/mod.rs`; it is wired into **no** engine read/commit path on `main`
  yet, so even a residual bug could not affect live reads today. No `unsafe`.
  Backend `get` errors propagate via `?` before populate → a failed fetch never
  poisons the cache.
- [OPERATIONAL → N/A] Read-side only; no GC/version-pin/format-migration
  interaction; fully reversible (a config flag).
- [SECURITY/OSS → CLEAN] No `Cargo.toml`/`Cargo.lock` change, no new dependency,
  no `unsafe`, no crafted-input surface (the fence is a `u64` comparison).

Non-blocking follow-ups (do NOT block; tracked):
- [CONTRACT] The fence is load-bearing on the ordering "external invalidate
  fires *strictly after* the new bytes are visible in the backend." The wrapper's
  own `put`/`delete` honor write-through-then-invalidate (cache.rs L441-453 /
  L526-535); the only external invalidator is the future commit path, which does
  not exist on `main`. Already tracked in T-0040 `deps: [..., BUG-0017]` with the
  fence dependency noted. T-0040 must add a test asserting invalidate-after-write
  ordering when it wires the commit path.
- [PROCESS/Cat.12] Decision-number collision: three files share `0034-*` in
  `.project/decisions/` (`bug-0017-...`, `t-0008-...`, `unicode-3-0-...`). Cosmetic
  / hygiene only — does not affect this fix. Recommend a renumber sweep
  (BUG-0010-style) by docs-memory-curator; not filing a new BUG since collisions
  are already a known, separately-tracked hygiene class.

Pre-mortem checkbox: APPROVED (the PR.md gate artifact is gone with the worktree;
this Notes entry is the equivalent committed sign-off record).
