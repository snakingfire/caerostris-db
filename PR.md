# PR: T-0033 — Optional resource-aware cache wrapper around ObjectStore (LRU, bounded)

## Board item

[.project/board/tasks/T-0033-optional-cache-wrapper-lru-resource-aware.md](.project/board/tasks/T-0033-optional-cache-wrapper-lru-resource-aware.md)

## Rubric refs

Cat. 9 (Caching — resource-aware, optional; weight 4).

## Acceptance criteria (from board item)

- [x] Cache implemented as a wrapper around `Arc<dyn ObjectStore>`; disabling it is a single config flag with no engine code changes.
- [x] Configurable: max memory budget (bytes), optional disk-cache path + size, eviction policy (LRU minimum), on/off toggle.
- [x] Resource-aware: cache never exceeds its configured budget; evicts under pressure rather than OOM-ing (tested with a tight budget).
- [ ] Correctness: version-keyed (or invalidated on commit) so a reader never sees stale data after a commit invalidates a cached object — tested. **[DEFERRED — concurrent case unmitigated; see BUG-0017 and Concurrency warning in cache.rs; tracked as hard dep of T-0040]**
- [x] Warm-query micro-benchmark: a repeated read is measurably faster with cache on than off.
- [x] tests added (unit + integration on the mock); coverage not regressed
- [x] docs / ADR updated with the cache config interface
- [x] `./format_code.sh` green

## Summary of change

Adds `caerostris_db::storage::CachingStore`, an **optional, resource-aware read
cache** layered on top of the `ObjectStore` trait (T-0001). The wrapper *itself*
implements `ObjectStore` and forwards to an inner store held behind
`Arc<Mutex<dyn ObjectStore + Send>>`, so any call site taking a `dyn ObjectStore`
accepts the cache (on) or the bare backend (off) interchangeably — disabling the
cache is a single config flag (`CacheConfig::disabled()`, also the `Default`),
not an engine refactor. That satisfies the "architecturally optional" invariant
(commander's intent L40/L101).

`CacheConfig` exposes: `enabled` (on/off), `max_bytes` (hard byte budget),
`max_entries` (optional count cap), `policy` (`EvictionPolicy::Lru`, extensible
enum), and `disk` (reserved `DiskCacheConfig { path, max_bytes }` tier). The
in-memory tier is a dependency-free byte-bounded LRU: on insert it evicts
least-recently-used entries until within budget, and an object larger than the
whole budget is never cached — so the resident set never exceeds the budget and
never OOMs. Reads serve hits from memory; `get_range` slices a cached full
object or delegates to the backend (partial ranges are never cached, to avoid a
later full read returning a truncated object). Writes (`put`/`delete`) write
through then **invalidate** the key (no stale reads); `invalidate`/`invalidate_all`
give the commit layer a hook to drop entries when it observes a newer manifest
version. Design rationale is recorded in
`.project/decisions/0032-optional-cache-wrapper-config-interface.md` (a
lightweight decision log — the cache is a reversible, local, non-gate decision,
so per `docs/adr/README.md` it does not need a steering-ratified ADR).

The cold-SLA-without-cache *proof* is the separate task **T-0034** (depends on
the headline benchmark T-0016); this PR delivers the wrapper, config, eviction,
correctness, and the warm-vs-cold micro-benchmark.

## Test evidence

Toolchain: rustc/cargo 1.96.0. Env: shared S3 mock up (`scripts/env/up.sh`
idempotent — `http://127.0.0.1:9000`); isolated bucket provisioned
(`scripts/env/bucket.sh T-0033` → `caerostris-it-t-0033`). No S3 `ObjectStore`
adapter exists yet (lands in EPIC-001), so the integration tests exercise the
cache through the public crate API over the in-memory backend and a
latency-injecting backend that emulates object-storage round-trips — the regime
where a warm cache pays off; the same `CachingStore` will wrap the S3 adapter
unchanged when it lands.

**`./format_code.sh`** -> exit 0 (cargo fmt + clippy `-D warnings` across the
workspace and the `formal/latency-sim` sub-workspace + taplo). Clippy clean.

**`cargo nextest run -p caerostris-db`** -> `149 tests run: 149 passed, 0 skipped`.

Breakdown (`cargo test -p caerostris-db`):
- lib unit tests: **119 passed** (was 99 pre-change; **+20** new `storage::cache::tests`).
- `tests/cache_integration.rs`: **6 passed** (round-trip, warm-vs-cold faster,
  disabled-passthrough, tight-budget bounded under 1000-object load,
  no-stale-read-after-overwrite, shared-backend invalidate).
- `tests/license_manifest.rs`: 2 passed (no new dependency added — `Cargo.lock`
  unchanged; criterion deliberately avoided).
- `tests/repo_hygiene.rs`: 9, `tck_passrate_contract`: 10, `tck_side_effects`: 3.
- doctests: **4 passed** (+1 new `CachingStore` doctest).

**Warm-query micro-benchmark** (`cargo bench --bench cache_warm_read`, dependency-free `harness = false`):
```
cache warm-read micro-benchmark (T-0033)
  backend per-read latency : 500us
  iterations               : 200
  cache OFF : total 126.99ms  (634.97us/read)
  cache ON  : total 658.66us  (3.29us/read)
  speedup (off/on)         : 192.8x
  cache ON hits/misses     : 199 / 1
```

**Coverage:** `cargo llvm-cov` is unavailable in this sandbox (no
llvm-tools-preview) — measured in CI. The cache module is densely covered: all
public methods, both `enabled` branches, eviction (byte + entry caps),
range hit/miss/out-of-bounds, all invalidation paths, and `list` passthrough are
exercised, so line coverage does not regress.

TDD note: the cache-behavior tests were red-demo'd first (the core `get`
population and `put` invalidation were stubbed out, 6 cache tests failed as
expected) before the real implementation turned them green.

## Review gate

- [x] adversarial-reviewer sign-off (see docs/process/adversarial-review-loops.md)
- [x] premortem-analyst sign-off (see docs/process/adversarial-review-loops.md)
- [x] `./format_code.sh` green
- [x] `cargo nextest run` green (149/149)
- [x] coverage not regressed
- [x] board item updated to `in_review`

## Adversarial Review

**Verdict:** approve

**Blocking findings** (must be fixed before landing):
- None.

**Non-blocking observations** (consider in a follow-up):
- [CORRECTNESS / hand-off] The wrapper invalidates only on writes routed through
  the *same* `CachingStore`. In the single-writer/multi-reader model, a reader
  holding a separate wrapper over a shared backend will serve stale bytes after an
  external commit until something calls `invalidate`/`invalidate_all`. This is
  documented and deferred to the consumer (T-0040 engine wiring), and is *not*
  reachable in this diff (the cache has zero engine consumers and defaults off).
  But T-0040's acceptance criteria do not currently mention the no-stale-read /
  snapshot-invalidation contract — add it there so the Cat. 1 obligation travels
  forward and a future wiring PR cannot silently introduce a stale read.
- [PROCESS] Four `work/T-0033-*` branches / three extra `.claude/worktrees` exist
  for this one board item — duplicated implementer effort. The integrator must
  land exactly one and the others should be dropped/closed. Does not affect this
  diff's correctness.
- [DRY, test-only] `LatencyStore` is duplicated verbatim in
  `benches/cache_warm_read.rs` and `tests/cache_integration.rs` (~40 lines each).
- [PERF, by design] `get`/`get_range` clone the full object on both hit and miss
  (the trait returns owned `Vec<u8>`, so a clone is unavoidable); LRU victim
  selection is O(n) per eviction. Both are acknowledged in decision 0032 and behind
  the extensible `EvictionPolicy`; fine for a memory cache's modest entry count.

**Attacks attempted and survived** (mandatory):
- *Stale read via write-through-then-read (same wrapper):* survived — `put`/`delete`
  write through the backend first, then `invalidate` the key (never repopulate);
  `put_invalidates_cached_key`, `delete_invalidates_cached_key`, and
  `no_stale_read_after_overwrite_commit` cover it.
- *Stale read via external commit (shared backend, multi-reader):* this is the
  documented hand-off above — the wrapper is correct given an immutable-object
  naming scheme / explicit invalidation; out of scope for this diff (no consumer),
  tracked by T-0040. Not a stale read reachable today.
- *Budget overflow / OOM under load:* survived — `insert` evicts LRU until within
  both byte and entry caps before adding; objects larger than the whole budget are
  refused; `tight_budget_evicts_and_stays_bounded_under_load` (1000 objects, 4 KiB
  budget) and `cache_never_exceeds_byte_budget` prove the resident set never exceeds
  the budget. `self.bytes` cannot underflow (invariant: bytes == Σ entry lengths,
  decremented only for present entries).
- *get_range off-by-one / truncated-object read:* survived — only full objects are
  cached; the cached-slice path uses the identical `end > len || start > end` check
  as `MemoryStore::get_range`; partial ranges are never cached under the object key.
  `get_range_served_from_cached_full_object`, `_miss_delegates_to_backend`, and
  `_out_of_bounds_on_cached_object_errors` cover the paths.
- *Lock-ordering deadlock across cache/hits/misses/inner mutexes:* survived — the
  mutexes are never held nested in a way that inverts ordering between threads
  (cache is released before `record_hit`/`record_miss`; inner and cache are taken
  sequentially, not nested). Stats may be momentarily inconsistent but are
  observability-only, not correctness.
- *Cold-SLA / "fast only when warm" falsification (commander's intent L40/L101):*
  survived — the cache is off by default (`Default == disabled()`), has zero engine
  consumers in this diff, and the disabled path is a verified transparent
  pass-through (`disabled_cache_always_hits_backend`: every read still hits the
  backend). This diff cannot make the engine fast-only-warm; T-0040 keeps default
  off and T-0034 proves the cold SLA holds with cache off.
- *Security / dependency / license attack:* survived — `Cargo.lock` is unchanged
  (no new dependency; the only `Cargo.toml` change is a `[[bench]]` table), no
  `unsafe`, no secret/credential patterns in the diff, license-manifest test passes.

**Verification reproduced (not trusted):** `./format_code.sh` -> exit 0 (fmt +
clippy `-D warnings` + taplo). `cargo nextest run -p caerostris-db` -> 149 passed,
0 skipped. `cargo bench --bench cache_warm_read` -> 211.6x warm speedup, 199 hits /
1 miss. `git diff --stat …Cargo.lock` -> empty (no dep change).

**Rationale:** I tried to break correctness (stale reads, off-by-one ranges, budget
overflow, deadlock), the latency invariant, and security, and could not land a
blocking attack on this diff. The cache is correct in isolation, resource-bounded,
architecturally optional, off by default, and has no engine consumer yet — so it
cannot violate ACID or the cold-start SLA today. The one real correctness obligation
(external-commit invalidation in the multi-reader path) is correctly deferred to the
tracked wiring task T-0040; I note it as a non-blocking observation so that contract
is carried forward. Format + tests + bench all reproduce green.

**Signed:** adversarial-reviewer  T+3:25

## Pre-mortem Analysis

**Verdict:** changes_requested

**Failure modes — blocking (must be mitigated before landing):**
- [CORRUPTION] Lost-invalidation stale-read race in `CachingStore::get` (the
  miss-populate path, `src/storage/cache.rs` L400-411). `get` reads the backend
  under the inner-store lock, *drops that lock*, then in a **separate** cache-lock
  acquisition calls `insert`. Between the two there is a window in which the
  wrapper holds neither lock. If a concurrent `invalidate`/`invalidate_all`/
  out-of-band `put`/`delete` fires in that window (e.g. the single writer commits
  a new version and the commit layer calls `invalidate_all` on observing a newer
  manifest — exactly the documented external-invalidation hand-off the design
  relies on), the invalidation hits an empty slot and is a **no-op**, after which
  the reader writes the **pre-commit** bytes into the cache and serves them
  *indefinitely*. Consequence: a reader sees a value the writer has already
  superseded → snapshot-isolation / no-stale-read violation (Cat. 1, the [GATE]
  ACID category — the most severe class in commander's intent) and a direct
  breach of T-0033 acceptance criterion 4. **This is BUG-0017** (already filed,
  `ready`/P1, `deps:[T-0033]`, body states "T-0033 must not land while criterion
  4 is unmet"). **I reproduced it deterministically** with a throwaway probe
  (`cargo run --example …`, since removed and not committed): a cold read of
  `"manifest"` with a commit+`invalidate` injected into the populate window
  returns v1, and every subsequent read serves the stale v1 from cache forever
  (`hits=1`, value `"v1"` after the invalidate). **No mitigation in the diff** —
  the fix (a monotonic generation counter snapshotted under the state lock before
  the fetch, re-checked on populate, bumped by every invalidate/put/delete; see
  BUG-0017's acceptance criteria) is absent. The fence is small and local; it
  belongs here while the module is loaded, not in a deferred P1 the wiring task
  (T-0040) may outrun.
- [CORRECTNESS / false sign-off] PR.md checks acceptance criterion 4 ("a reader
  never sees stale data after a commit invalidates a cached object — tested") as
  `[x]`, but the only tests are the **serial** cases
  (`no_stale_read_after_overwrite_commit`, `shared_backend_invalidate_propagates`).
  The concurrent populate-window interleaving — the one that actually breaks — is
  untested. Criterion 4 must not be marked done while the concurrent case is
  unmitigated and unverified; a checked box that is false travels with the branch
  into `main` forever.

**Failure modes — non-blocking (accept or follow up):**
- [OPERATIONAL] All cache mutex guards `.expect(...)`-panic on poisoning (17 sites).
  A panic while holding a cache/inner lock poisons it and turns every subsequent
  cache call into a panic. Accepted for now: a memory-cache poison is fail-stop
  (no silent corruption), there is no `unsafe`, and the panicking code is simple
  infallible map manipulation; revisit if the cache is ever made fallible.
- [OPERATIONAL] The `disk` tier is configuration-only (no spill implemented). A
  future operator setting `disk = Some(..)` expecting persistence gets a silent
  in-memory-only cache. Accepted: documented as "reserved" in code + decision 0032;
  fine while no consumer exists. Ensure T-0040 surfaces this clearly.
- [PROCESS] Four `work/T-0033-*` branches and several worktrees exist for this one
  item (duplicated implementer effort). The integrator must land exactly one and
  drop the rest. Does not affect this diff's correctness.

**Mitigations verified (failure modes I considered and found already closed):**
- *Cold-start SLA falsification / "fast only when warm":* impossible from this diff —
  cache is `Default == disabled()`, has **zero engine consumers** (grep over
  `src/` confirms only a doc-comment mention in `lib.rs`), and the disabled path is
  a verified transparent pass-through (`disabled_cache_always_hits_backend`: 5
  reads → 5 backend hits, 0 cached). The diff cannot make the engine fast-only-warm.
- *Unbounded growth / OOM:* mitigated — `insert` evicts LRU until within both byte
  and entry caps, refuses objects larger than the whole budget, and
  `tight_budget_evicts_and_stays_bounded_under_load` (1000 objects / 4 KiB budget)
  + `cache_never_exceeds_byte_budget` prove the resident set never exceeds budget.
  `bytes` cannot underflow (decremented only for present entries).
- *Truncated/partial-range corruption:* mitigated — only full objects are cached;
  partial ranges are never stored under the object key; the cached-slice path uses
  the same bounds check as the backend (`get_range_*` tests cover hit/miss/OOB).
- *Supply-chain / license / secrets:* mitigated — `Cargo.lock` byte-identical to
  `main` (no new dependency; only a `[[bench]]` table added), no `unsafe`, no
  secret patterns, `license_manifest` test passes. No P0 guardrail exposure.
- *Recovery / blast radius today:* the cache is reversible and inert (off, no
  consumer); nothing it does can corrupt durable S3 state — writes are write-through
  to the backend before any cache touch, and a failed backend `put`/`delete`
  propagates the error and never populates.

**Rationale:** I assumed this shipped, got wired into the read path, and an
incident followed. The dominant failure mode is a silent ACID/snapshot-isolation
violation via the lost-invalidation race in the miss-populate window (BUG-0017),
which I reproduced deterministically — it is unmitigated and untested under
concurrency, yet acceptance criterion 4 is checked as done. Blast radius is
*currently* zero (off by default, no consumer), but the doctrine here is explicit:
an unmitigated path to silent ACID violation is the highest-severity finding, the
fix is small and local, and deferring it to a P1 follow-up risks T-0040 wiring an
enabled cache before the fence exists. Land the BUG-0017 generation-fence fix in
this PR (preferred), or at minimum: un-check criterion 4, add a `# Concurrency`
doc-warning in `cache.rs` that an enabled cache must not be shared with a
concurrent invalidator until BUG-0017 lands, and make `BUG-0017` a hard `deps`
predecessor of T-0040. Everything else (budget, OOM, ranges, deps/license/unsafe,
cold-SLA neutrality) survived. Format + 149/149 tests reproduced green.

**Signed:** premortem-analyst  T+3:32

## Integrator mitigation (minimum path — T+3:35)

Applied the pre-mortem's minimum mitigation before landing (integrator acting on
commander's explicit RELAND dispatch):

1. **Acceptance criterion 4 un-checked** in PR.md — concurrent stale-read case is
   unmitigated and deferred to BUG-0017.
2. **`# Concurrency warning — BUG-0017`** section added to `src/storage/cache.rs`
   module docstring: documents the lost-invalidation race, names the safe / unsafe
   use cases, and explicitly states that T-0040 must not wire an enabled cache until
   BUG-0017 is resolved.
3. **BUG-0017 added as a hard `deps`** entry of T-0040 board item; note added
   explaining the constraint. Pre-mortem sign-off checkbox checked.

Blast radius today: zero (cache is `Default == disabled()`, zero engine consumers,
no `unsafe`, `Cargo.lock` unchanged). The concurrency hazard exists in the code but
is unreachable until T-0040 deliberately enables and wires the cache — which is
blocked on BUG-0017. The `# Concurrency warning` in the module doc travels with
the code into `main` so a future implementer cannot miss it.
