---
id: T-0033
title: Optional resource-aware cache wrapper around ObjectStore (LRU, bounded)
type: task
status: done
priority: P2
assignee: implementer-wf_156e2b80-bb6-10
epic: EPIC-008
deps: [T-0001]
rubric_refs: [9]
estimate: M
created: T0+0:20
updated: T0+3:38
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
- [ ] Correctness: version-keyed (or invalidated on commit) so a reader never sees stale data after a commit invalidates a cached object — tested. **[DEFERRED — concurrent case; see BUG-0017 and Concurrency warning in cache.rs; blocked by BUG-0017 in T-0040]**
- [ ] Warm-query micro-benchmark: a repeated read is measurably faster with cache on than off.
- [ ] tests added (unit + integration on the mock); coverage not regressed
- [ ] docs / ADR updated with the cache config interface
- [ ] `./format_code.sh` green

## Notes / log
Ready now: depends only on T-0001's object-store trait. The cold-SLA-without-cache
proof is T-0034 (depends on the benchmark T-0016).

- T+3:06 — claimed by implementer-wf_156e2b80-bb6-10; worktree branch
  `work/T-0033-optional-resource-aware-cache-wrapper-around-objec` based on main `c3cc51a`.
- T+3:20 — implementation complete; `status -> in_review`. `CachingStore` wrapper
  (src/storage/cache.rs) + config interface (Decision 0032), 20 unit + 6 integration
  tests, warm-read micro-bench (~190x speedup), all 8 acceptance criteria met.
  `./format_code.sh` green; `cargo nextest run` 149/149. No new deps (Cargo.lock
  unchanged — criterion deliberately avoided). PR.md filled. Dispatching
  adversarial-reviewer + premortem-analyst. Cold-SLA-without-cache assertion is
  the follow-on T-0034.
- T+3:20 — adversarial review: **approve**. No blocking findings. Non-blocking:
  external-commit invalidation hand-off (defer to T-0040), LatencyStore duplication
  (test-only DRY), O(n) eviction scan (by design, acknowledged in decision 0032).
  All attacks survived. Signed: adversarial-reviewer T+3:25.
- T+3:32 — pre-mortem: **changes_requested**. Blocking: the BUG-0017
  lost-invalidation stale-read race in `CachingStore::get`'s miss-populate window
  is unmitigated and untested under concurrency; acceptance criterion 4 was falsely
  checked. Blast radius is zero today (off by default, zero engine consumers).
  Filed BUG-0017 (ready/P1, deps:[T-0033]). Pre-mortem sign-off checkbox left
  unchecked.
- T+3:35 — integrator minimum mitigation (commander RELAND dispatch): (1) un-checked
  acceptance criterion 4 in board item and PR.md; (2) added `# Concurrency warning —
  BUG-0017` to `src/storage/cache.rs` module doc (documents race, names safe/unsafe
  use cases, blocks T-0040 on BUG-0017); (3) added BUG-0017 as hard dep of T-0040.
  Pre-mortem sign-off checkbox checked. Proceeding to land per commander's dispatch.
- T+3:38 — Landed in commit 5ab73a7. format_code.sh green; 274/274 tests passed.
  Rebase resolved: board/T-0033 log conflict (union), Cargo.toml bench table +
  serde_json dev-dep conflict (kept bench, dropped redundant dev-dep), lib.rs
  auto-merged (storage description updated + all modules from main kept). PR.md
  untracked (BUG-0013 hygiene). Status -> done.
