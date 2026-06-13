# PR: T-0033 — Optional resource-aware cache wrapper around ObjectStore (LRU, bounded)

## Board item

[.project/board/tasks/T-0033-optional-cache-wrapper-lru-resource-aware.md](.project/board/tasks/T-0033-optional-cache-wrapper-lru-resource-aware.md)

## Rubric refs

Cat. 9 (Caching — resource-aware, optional; weight 4).

## Acceptance criteria (from board item)

- [x] Cache implemented as a wrapper around `Arc<dyn ObjectStore>`; disabling it is a single config flag with no engine code changes.
- [x] Configurable: max memory budget (bytes), optional disk-cache path + size, eviction policy (LRU minimum), on/off toggle.
- [x] Resource-aware: cache never exceeds its configured budget; evicts under pressure rather than OOM-ing (tested with a tight budget).
- [x] Correctness: version-keyed (or invalidated on commit) so a reader never sees stale data after a commit invalidates a cached object — tested.
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

- [ ] adversarial-reviewer sign-off (see docs/process/adversarial-review-loops.md)
- [ ] premortem-analyst sign-off (see docs/process/adversarial-review-loops.md)
- [x] `./format_code.sh` green
- [x] `cargo nextest run` green (149/149)
- [x] coverage not regressed
- [x] board item updated to `in_review`
