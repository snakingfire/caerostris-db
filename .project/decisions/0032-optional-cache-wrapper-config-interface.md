# Decision 0032 — Optional resource-aware cache wrapper: config interface

- **Type:** lightweight decision log (reversible, local, implementation-level —
  not a steering-gated ADR; see `docs/adr/README.md`).
- **Date / T+ marker:** 2026-06-13T21:40:00Z (≈ T+3:16)
- **Owner:** implementer (T-0033)
- **Board item:** `.project/board/tasks/T-0033-optional-cache-wrapper-lru-resource-aware.md`
- **Epic:** EPIC-008 — Resource-aware optional caching
- **Rubric:** Cat. 9 (caching, weight 4)

## Context

Cat. 9 calls for a *configurable, resource-aware, optional* local cache that
speeds warm queries while the cold-start SLA holds with the cache **off**
(commander's intent L40/L101). The cache must wrap the `ObjectStore` abstraction
(T-0001) and be architecturally optional: disabling it is a single config flag,
not an engine refactor. The cold-SLA-without-cache *proof* is a separate task
(T-0034, depends on the headline benchmark T-0016); this task delivers the
wrapper, its config interface, eviction, correctness, and a warm-vs-cold
micro-benchmark.

## Decision

We add `caerostris_db::storage::CachingStore`, a wrapper that **implements
`ObjectStore`** and forwards to an inner store held behind
`Arc<Mutex<dyn ObjectStore + Send>>`. Because the wrapper *is* an `ObjectStore`,
any code that takes a `dyn ObjectStore` accepts the cache (on) or the bare
backend (off) interchangeably — that substitutability is what makes the cache
optional with no engine code change.

### Config interface (`CacheConfig`)

| Field | Type | Meaning |
|-------|------|---------|
| `enabled` | `bool` | Master on/off. `false` ⇒ transparent pass-through; nothing cached. |
| `max_bytes` | `usize` | Hard ceiling on resident cached bytes (sum of object sizes). |
| `max_entries` | `Option<usize>` | Optional cap on number of cached objects. |
| `policy` | `EvictionPolicy` | Eviction policy; `Lru` today (enum is extensible). |
| `disk` | `Option<DiskCacheConfig>` | Reserved on-disk tier (`path` + `max_bytes`); surface is stable, in-memory tier is the active one. |

Constructors: `CacheConfig::disabled()` (also `Default`), `with_memory_budget(bytes)`,
and a `.with_max_entries(n)` builder.

### Behaviour

- **Reads** (`get`): hit ⇒ served from memory (no backend call); miss ⇒ fetch
  from backend, insert under the byte budget, return.
- **`get_range`**: if the full object is cached, slice locally (validating the
  range exactly as the backend would); otherwise delegate to the backend.
  Partial ranges are **not** cached under the object key (would risk a later full
  read returning a truncated object).
- **Writes** (`put`/`delete`): write through to the backend, then **invalidate**
  the affected key in the cache (never repopulate) — so a reader never sees a
  pre-write value.
- **External mutation** (e.g. a commit by the single writer not issued through
  this wrapper): callers invalidate explicitly via `invalidate(key)` /
  `invalidate_all()`. The storage/commit layer calls these when it observes a
  newer manifest version, keeping the cache version-correct.
- **`list`**: never cached (cheap relative to object bytes; changes on every
  mutation) — always reflects the backend.
- **Resource-awareness:** on insert, evict LRU entries until within both
  `max_bytes` and `max_entries`. An object larger than the whole budget is never
  cached (it cannot fit without violating the budget). The resident set therefore
  never exceeds the budget — no unbounded growth, no OOM. `CacheStats` exposes
  hits / misses / evictions / entries / bytes.

## Alternatives considered

- **Bake caching into `ObjectStore` / `MemoryStore`.** Rejected: it would make
  the cache non-optional and couple every backend to caching logic, violating the
  "single config flag, no refactor" requirement.
- **Use the `lru` / `moka` / `cached` crates.** Rejected for now: each drags a
  transitive dependency tree that must be license-recorded (Cat. 12) for a small,
  auditable byte-bounded LRU we can implement in ~120 lines with zero deps. The
  policy is behind `EvictionPolicy`, so swapping in a library later is local.
- **Criterion for the warm-query benchmark.** Rejected: criterion's tree
  (plotters, rayon, regex, …) is heavy for one micro-measurement and would expand
  the license manifest substantially. Instead the bench is a dependency-free
  `harness = false` binary (`benches/cache_warm_read.rs`), and the warm-vs-cold
  property is *also* asserted as a CI test in `tests/cache_integration.rs`.
- **Version-keyed cache keys (`(key, version)`).** Deferred: the manifest/commit
  protocol that supplies versions is still landing. `invalidate*` gives the
  commit layer a clean hook today; a future version-keyed variant can be added
  behind the same wrapper without changing callers.

## Consequences

### Positive

- Cat. 9: configurable, resource-aware, measurably-faster-warm cache that is off
  by default and optional by construction.
- Zero new runtime dependencies; small, auditable surface.
- Drop-in over any future backend (S3 adapter from EPIC-001) via the trait.

### Negative / trade-offs

- LRU eviction victim selection is O(n) per eviction (linear scan of entries).
  Fine for a memory cache's modest entry count; if it ever matters, replace the
  internal structure behind `EvictionPolicy` without touching the public API.
- The `disk` tier is configuration-only today (no spill implementation yet).
- `Arc<Mutex<…>>` serialises backend access through one lock; acceptable for the
  single-writer / multi-reader model and avoids requiring interior mutability in
  every backend. Revisit if read concurrency becomes a bottleneck.

### Open questions

- Version-keyed caching once the commit/manifest version is wired (handoff to the
  storage layer; `invalidate_all` on newer-manifest observation is the interim).
- The cold-SLA-without-cache assertion is T-0034 (needs benchmark T-0016).

## Rubric impact

| Cat. | Name | Impact |
|------|------|--------|
| 9 | Caching | Wrapper + config + eviction + correctness + warm-vs-cold bench → toward 100 (cold-SLA proof remains T-0034). |
