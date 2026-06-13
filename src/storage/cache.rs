//! Optional, resource-aware read cache that wraps an [`ObjectStore`].
//!
//! [`CachingStore`] is a thin, *optional* accelerator layered **on top of** the
//! [`ObjectStore`] trait (T-0001). It speeds up repeated reads of the same
//! object by holding recently-read bytes in a byte-bounded, LRU-evicted
//! in-memory cache. It is **never** required for correctness or for the
//! cold-start latency SLA: turning it off is a single config flag
//! ([`CacheConfig::disabled`]) and changes no engine code (commander's intent
//! L40/L101; Cat. 9). The cold-SLA-without-cache proof lives in T-0034.
//!
//! ## Why a wrapper, not a trait change
//!
//! The cache implements [`ObjectStore`] itself and forwards to an inner store.
//! Because it *is* an `ObjectStore`, any code that already takes a
//! `dyn ObjectStore` can be handed a `CachingStore` (cache on) or the bare
//! backend (cache off) interchangeably — the rest of the engine never learns
//! whether a cache is present. That is what makes the cache *architecturally
//! optional*.
//!
//! ## Resource-awareness
//!
//! The in-memory cache is bounded by a configured **byte budget**
//! ([`CacheConfig::max_bytes`]) and an optional **entry-count cap**. On every
//! insert it evicts the least-recently-used entries until it is back within
//! budget, so it never grows without bound and never OOMs the host. An object
//! larger than the whole budget is simply not cached (it cannot fit without
//! violating the budget), and reads of it always go to the backend.
//!
//! ## Correctness (no stale reads)
//!
//! Every mutation routed through the wrapper — [`put`](ObjectStore::put) and
//! [`delete`](ObjectStore::delete) — invalidates the affected key in the cache
//! before returning, so a subsequent read never observes a pre-write value.
//! For mutations that happen *outside* this wrapper (e.g. a commit performed by
//! the single writer that this reader did not issue), callers invalidate
//! explicitly via [`CachingStore::invalidate`] / [`CachingStore::invalidate_all`].
//! The storage/commit layer is expected to call `invalidate_all` (or
//! per-object `invalidate`) when it observes a newer manifest version, keeping
//! the cache *version-correct*.
//!
//! ## Concurrency: the generation fence (BUG-0017)
//!
//! The miss-populate path in [`CachingStore::get`] fetches bytes from the backend
//! while holding **no** cache-state lock (so concurrent cold readers are not
//! serialised behind one another), and then re-acquires the cache lock to insert.
//! Naively, a concurrent `invalidate` / `invalidate_all` / `put` / `delete`
//! executing in that window against an empty cache slot is a no-op (the entry is
//! not yet inserted); the reader would then populate the cache with **pre-commit
//! bytes** and serve the stale version indefinitely — a snapshot-isolation
//! violation (Cat. 1 / ACID gate).
//!
//! That window is closed by a **monotonic generation counter** held inside the
//! cache state (so it moves under the very lock that guards the map). A cold read
//! snapshots the generation under the cache lock *before* the backend fetch;
//! every coherence-affecting mutation (`invalidate`, `invalidate_all`, and the
//! `invalidate` issued by `put`/`delete`) bumps it; the populate re-acquires the
//! lock and inserts the fetched bytes **only if the generation is unchanged**. If
//! a commit raced the fetch, the bytes are dropped rather than poisoning the
//! cache. The racing read still returns the bytes it fetched (a read serializable
//! *before* the commit), but the cache is never left holding a superseded
//! version. This makes the Arc-shared single-writer / multi-reader wiring target
//! (T-0040) safe to enable. See `docs/adr/0009-optional-resource-aware-cache.md`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::{ObjectStore, StoreError};

/// Cache eviction policy.
///
/// Only LRU is implemented today; the enum exists so additional policies
/// (e.g. LFU, segmented LRU) can be added without changing the config shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EvictionPolicy {
    /// Evict the least-recently-used entry first.
    #[default]
    Lru,
}

/// Optional on-disk cache tier configuration.
///
/// Reserved for a future spill-to-disk tier. It is part of the configuration
/// surface today (so the config interface is stable) but the in-memory tier is
/// the only one that is active; see the module docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskCacheConfig {
    /// Directory under which cached objects are spilled.
    pub path: std::path::PathBuf,
    /// Maximum number of bytes the on-disk tier may occupy.
    pub max_bytes: u64,
}

/// Configuration for the optional [`CachingStore`].
///
/// The cache is **off by default** ([`CacheConfig::default`] returns a disabled
/// config) so that "no config" means "no cache" — the safe, SLA-preserving
/// default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheConfig {
    /// Master on/off switch. When `false`, the wrapper is a transparent
    /// pass-through: every call forwards straight to the backend and nothing is
    /// cached.
    pub enabled: bool,
    /// Maximum bytes the in-memory cache may hold (sum of cached object sizes).
    pub max_bytes: usize,
    /// Optional cap on the number of cached objects, independent of bytes.
    pub max_entries: Option<usize>,
    /// Eviction policy used when the cache is over budget.
    pub policy: EvictionPolicy,
    /// Optional on-disk cache tier (reserved; see [`DiskCacheConfig`]).
    pub disk: Option<DiskCacheConfig>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

impl CacheConfig {
    /// A fully-disabled cache: the wrapper is a transparent pass-through.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            max_bytes: 0,
            max_entries: None,
            policy: EvictionPolicy::Lru,
            disk: None,
        }
    }

    /// An enabled in-memory LRU cache bounded to `max_bytes` bytes.
    #[must_use]
    pub fn with_memory_budget(max_bytes: usize) -> Self {
        Self {
            enabled: true,
            max_bytes,
            max_entries: None,
            policy: EvictionPolicy::Lru,
            disk: None,
        }
    }

    /// Builder: cap the number of cached entries in addition to the byte budget.
    #[must_use]
    pub fn with_max_entries(mut self, max_entries: usize) -> Self {
        self.max_entries = Some(max_entries);
        self
    }
}

/// Snapshot of cache activity counters, for tests, benches, and observability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CacheStats {
    /// Reads served from the cache without touching the backend.
    pub hits: u64,
    /// Reads that missed the cache and went to the backend.
    pub misses: u64,
    /// Entries evicted to stay within budget.
    pub evictions: u64,
    /// Current number of cached objects.
    pub entries: usize,
    /// Current total bytes held by cached objects.
    pub bytes: usize,
}

/// Internal: a byte-bounded LRU map from object key to cached bytes.
///
/// Recency is tracked by a monotonically increasing `tick`; the entry with the
/// smallest `last_used` tick is the eviction victim. This is O(n) per eviction
/// scan, which is fine for the modest entry counts a memory cache holds and
/// keeps the implementation dependency-free and easy to audit.
#[derive(Debug)]
struct LruByteCache {
    max_bytes: usize,
    max_entries: Option<usize>,
    bytes: usize,
    clock: u64,
    map: HashMap<String, CacheEntry>,
    evictions: u64,
    /// Monotonic generation counter, bumped by every coherence-affecting
    /// mutation (`remove` / `clear`, i.e. the invalidation paths). A cold read
    /// snapshots it before the backend fetch and only populates if it is
    /// unchanged afterwards — the lost-invalidation fence (BUG-0017). Eviction
    /// (`evict_one`) deliberately does **not** bump it: evicting a stale-or-fresh
    /// entry to reclaim space is not an invalidation and must not fence out an
    /// in-flight populate of a *different* key.
    generation: u64,
}

#[derive(Debug)]
struct CacheEntry {
    bytes: Arc<Vec<u8>>,
    last_used: u64,
}

impl LruByteCache {
    fn new(max_bytes: usize, max_entries: Option<usize>) -> Self {
        Self {
            max_bytes,
            max_entries,
            bytes: 0,
            clock: 0,
            map: HashMap::new(),
            evictions: 0,
            generation: 0,
        }
    }

    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    /// Current generation. Snapshotted (under the cache lock) before a backend
    /// fetch so the populate can detect a racing invalidation.
    fn generation(&self) -> u64 {
        self.generation
    }

    /// Bump the generation, fencing out any in-flight populate that snapshotted
    /// an earlier value. Called by the invalidation paths only.
    fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// Populate the cache on a cold-read miss, but only if no invalidation has
    /// run since `snapshot` was taken. Returns `true` if the bytes were cached.
    ///
    /// This is the BUG-0017 fence: if a commit's `invalidate`/`invalidate_all`
    /// (or the `invalidate` issued by a concurrent `put`/`delete`) bumped the
    /// generation while the reader was fetching from the backend, the fetched
    /// bytes are dropped rather than caching a version that has just been
    /// superseded.
    fn insert_if_current(&mut self, key: &str, value: Arc<Vec<u8>>, snapshot: u64) -> bool {
        if self.generation != snapshot {
            return false;
        }
        self.insert(key, value);
        true
    }

    /// Fetch a cached object, refreshing its recency. Returns `None` on a miss.
    fn get(&mut self, key: &str) -> Option<Arc<Vec<u8>>> {
        let now = self.tick();
        let entry = self.map.get_mut(key)?;
        entry.last_used = now;
        Some(Arc::clone(&entry.bytes))
    }

    /// Insert (or replace) an object, evicting LRU entries to stay in budget.
    ///
    /// An object that cannot fit within `max_bytes` even when the cache is
    /// otherwise empty is *not* stored — caching it would violate the budget.
    fn insert(&mut self, key: &str, value: Arc<Vec<u8>>) {
        let size = value.len();

        // Remove any existing entry for this key first so its bytes don't
        // double-count and so a replace re-establishes recency.
        self.remove(key);

        // Refuse to cache an object larger than the entire budget; the budget
        // is a hard ceiling and must never be exceeded.
        if size > self.max_bytes {
            return;
        }

        // Evict until inserting `size` bytes (and one more entry) stays in budget.
        while self.bytes + size > self.max_bytes
            || self.max_entries.is_some_and(|cap| self.map.len() + 1 > cap)
        {
            if !self.evict_one() {
                // Nothing left to evict but still over budget: bail without
                // caching rather than exceed the budget.
                return;
            }
        }

        let now = self.tick();
        self.bytes += size;
        self.map.insert(
            key.to_owned(),
            CacheEntry {
                bytes: value,
                last_used: now,
            },
        );
    }

    /// Evict the single least-recently-used entry. Returns `false` if empty.
    fn evict_one(&mut self) -> bool {
        let victim = self
            .map
            .iter()
            .min_by_key(|(_, e)| e.last_used)
            .map(|(k, _)| k.clone());
        match victim {
            Some(key) => {
                self.remove(&key);
                self.evictions += 1;
                true
            }
            None => false,
        }
    }

    /// Remove an entry if present, reclaiming its bytes.
    fn remove(&mut self, key: &str) {
        if let Some(entry) = self.map.remove(key) {
            self.bytes -= entry.bytes.len();
        }
    }

    fn clear(&mut self) {
        self.map.clear();
        self.bytes = 0;
    }
}

/// An optional, resource-aware read cache wrapping an [`ObjectStore`].
///
/// Construct it from any backend store with [`CachingStore::new`]. The inner
/// store is held behind `Arc<Mutex<…>>` so the wrapper can be shared
/// (`Arc<dyn ObjectStore>`) across reader threads while still forwarding the
/// trait's `&mut self` writes (`put`/`delete`) to the single writer — mirroring
/// the engine's single-writer / multi-reader model.
///
/// ```
/// use caerostris_db::storage::{CacheConfig, CachingStore, MemoryStore, ObjectStore};
///
/// let mut store = CachingStore::new(MemoryStore::new(), CacheConfig::with_memory_budget(1 << 20));
/// store.put("k", b"v".to_vec()).unwrap();
/// assert_eq!(store.get("k").unwrap(), b"v"); // miss → backend, then cached
/// assert_eq!(store.get("k").unwrap(), b"v"); // hit → served from cache
/// assert_eq!(store.stats().hits, 1);
/// ```
#[derive(Clone)]
pub struct CachingStore {
    inner: Arc<Mutex<dyn ObjectStore + Send>>,
    config: CacheConfig,
    cache: Arc<Mutex<LruByteCache>>,
    hits: Arc<Mutex<u64>>,
    misses: Arc<Mutex<u64>>,
}

impl std::fmt::Debug for CachingStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachingStore")
            .field("config", &self.config)
            .field("stats", &self.stats())
            .finish_non_exhaustive()
    }
}

impl CachingStore {
    /// Wrap `inner` with the given cache `config`.
    ///
    /// When `config.enabled` is `false` the wrapper is a transparent
    /// pass-through and allocates no cache storage beyond the (empty) map.
    pub fn new<S>(inner: S, config: CacheConfig) -> Self
    where
        S: ObjectStore + Send + 'static,
    {
        Self::from_arc(Arc::new(Mutex::new(inner)), config)
    }

    /// Wrap an already-shared `Arc<Mutex<dyn ObjectStore + Send>>`.
    ///
    /// Useful when several wrappers (or a writer and reader path) must share the
    /// same backing store handle.
    #[must_use]
    pub fn from_arc(inner: Arc<Mutex<dyn ObjectStore + Send>>, config: CacheConfig) -> Self {
        let cache = LruByteCache::new(config.max_bytes, config.max_entries);
        Self {
            inner,
            config,
            cache: Arc::new(Mutex::new(cache)),
            hits: Arc::new(Mutex::new(0)),
            misses: Arc::new(Mutex::new(0)),
        }
    }

    /// Whether caching is enabled for this wrapper.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// The configuration this wrapper was built with.
    #[must_use]
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }

    /// Invalidate a single key, dropping any cached bytes for it.
    ///
    /// Call this when an object is mutated outside this wrapper (e.g. the writer
    /// committed a new version) so readers never serve stale data. Also bumps the
    /// cache generation so an in-flight cold-read populate is fenced out
    /// (BUG-0017).
    pub fn invalidate(&self, key: &str) {
        let mut cache = self.cache.lock().expect("cache mutex poisoned");
        cache.remove(key);
        cache.bump_generation();
    }

    /// Invalidate the entire cache (e.g. on observing a newer manifest version).
    ///
    /// Bumps the generation so any in-flight cold-read populate is fenced out
    /// (BUG-0017).
    pub fn invalidate_all(&self) {
        let mut cache = self.cache.lock().expect("cache mutex poisoned");
        cache.clear();
        cache.bump_generation();
    }

    /// Current cache activity snapshot.
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        let cache = self.cache.lock().expect("cache mutex poisoned");
        CacheStats {
            hits: *self.hits.lock().expect("hits mutex poisoned"),
            misses: *self.misses.lock().expect("misses mutex poisoned"),
            evictions: cache.evictions,
            entries: cache.map.len(),
            bytes: cache.bytes,
        }
    }

    fn record_hit(&self) {
        *self.hits.lock().expect("hits mutex poisoned") += 1;
    }

    fn record_miss(&self) {
        *self.misses.lock().expect("misses mutex poisoned") += 1;
    }
}

impl ObjectStore for CachingStore {
    fn put(&mut self, key: &str, bytes: Vec<u8>) -> Result<(), StoreError> {
        // Write through to the backend first; only on success do we touch the
        // cache, and we *invalidate* (never populate) so a failed read-back
        // cannot resurrect a value the backend rejected.
        self.inner
            .lock()
            .expect("inner store mutex poisoned")
            .put(key, bytes)?;
        if self.config.enabled {
            self.invalidate(key);
        }
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, StoreError> {
        if !self.config.enabled {
            return self
                .inner
                .lock()
                .expect("inner store mutex poisoned")
                .get(key);
        }

        // Snapshot the generation under the cache lock before releasing it for
        // the (slow) backend fetch. On a hit we return immediately; on a miss we
        // carry the snapshot through to the populate below (BUG-0017 fence).
        let snapshot = {
            let mut cache = self.cache.lock().expect("cache mutex poisoned");
            if let Some(bytes) = cache.get(key) {
                self.record_hit();
                return Ok((*bytes).clone());
            }
            cache.generation()
        };

        self.record_miss();
        let bytes = self
            .inner
            .lock()
            .expect("inner store mutex poisoned")
            .get(key)?;
        let shared = Arc::new(bytes);
        // Populate only if no invalidation raced the fetch; otherwise drop the
        // bytes rather than caching a superseded version.
        self.cache
            .lock()
            .expect("cache mutex poisoned")
            .insert_if_current(key, Arc::clone(&shared), snapshot);
        Ok((*shared).clone())
    }

    fn get_range(&self, key: &str, start: usize, end: usize) -> Result<Vec<u8>, StoreError> {
        if !self.config.enabled {
            return self
                .inner
                .lock()
                .expect("inner store mutex poisoned")
                .get_range(key, start, end);
        }

        // If we have the whole object cached, slice it locally (validating the
        // range exactly as the backend would). We do *not* cache partial ranges
        // under the object key, since that would let a later full read return a
        // truncated object — a correctness hazard.
        if let Some(bytes) = self.cache.lock().expect("cache mutex poisoned").get(key) {
            let len = bytes.len();
            if end > len || start > end {
                return Err(StoreError::RangeOutOfBounds {
                    key: key.to_owned(),
                    object_len: len,
                    start,
                    end,
                });
            }
            self.record_hit();
            return Ok(bytes[start..end].to_vec());
        }

        self.record_miss();
        self.inner
            .lock()
            .expect("inner store mutex poisoned")
            .get_range(key, start, end)
    }

    fn delete(&mut self, key: &str) -> Result<(), StoreError> {
        self.inner
            .lock()
            .expect("inner store mutex poisoned")
            .delete(key)?;
        if self.config.enabled {
            self.invalidate(key);
        }
        Ok(())
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        // Listings are not cached: they change on every put/delete and are cheap
        // relative to object bytes. Always reflect the backend.
        self.inner
            .lock()
            .expect("inner store mutex poisoned")
            .list(prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemoryStore;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A backend that counts how many `get`/`get_range` calls reach it, so a
    /// test can prove a read was served from cache (no backend hit) vs. missed.
    #[derive(Default)]
    struct CountingStore {
        inner: MemoryStore,
        gets: Arc<AtomicUsize>,
        range_gets: Arc<AtomicUsize>,
    }

    impl CountingStore {
        fn new() -> Self {
            Self::default()
        }
        fn get_count(&self) -> usize {
            self.gets.load(Ordering::SeqCst)
        }
        fn range_count(&self) -> usize {
            self.range_gets.load(Ordering::SeqCst)
        }
    }

    impl ObjectStore for CountingStore {
        fn put(&mut self, key: &str, bytes: Vec<u8>) -> Result<(), StoreError> {
            self.inner.put(key, bytes)
        }
        fn get(&self, key: &str) -> Result<Vec<u8>, StoreError> {
            self.gets.fetch_add(1, Ordering::SeqCst);
            self.inner.get(key)
        }
        fn get_range(&self, key: &str, start: usize, end: usize) -> Result<Vec<u8>, StoreError> {
            self.range_gets.fetch_add(1, Ordering::SeqCst);
            self.inner.get_range(key, start, end)
        }
        fn delete(&mut self, key: &str) -> Result<(), StoreError> {
            self.inner.delete(key)
        }
        fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
            self.inner.list(prefix)
        }
    }

    // ---- config defaults -------------------------------------------------

    #[test]
    fn default_config_is_disabled() {
        assert!(!CacheConfig::default().enabled);
        assert!(!CacheConfig::disabled().enabled);
    }

    #[test]
    fn memory_budget_config_is_enabled() {
        let c = CacheConfig::with_memory_budget(4096);
        assert!(c.enabled);
        assert_eq!(c.max_bytes, 4096);
        assert_eq!(c.policy, EvictionPolicy::Lru);
    }

    #[test]
    fn with_max_entries_builder() {
        let c = CacheConfig::with_memory_budget(4096).with_max_entries(3);
        assert_eq!(c.max_entries, Some(3));
    }

    // ---- read-through caching --------------------------------------------

    #[test]
    fn second_read_is_served_from_cache() {
        let counting = CountingStore::new();
        let gets = Arc::clone(&counting.gets);
        let mut store = CachingStore::new(counting, CacheConfig::with_memory_budget(1 << 20));
        store.put("k", b"value".to_vec()).unwrap();

        assert_eq!(store.get("k").unwrap(), b"value");
        assert_eq!(store.get("k").unwrap(), b"value");
        // Only the first read reaches the backend; the second is a cache hit.
        assert_eq!(gets.load(Ordering::SeqCst), 1);
        let s = store.stats();
        assert_eq!(s.hits, 1);
        assert_eq!(s.misses, 1);
        assert_eq!(s.entries, 1);
    }

    #[test]
    fn disabled_cache_is_transparent_passthrough() {
        let counting = CountingStore::new();
        let gets = Arc::clone(&counting.gets);
        let mut store = CachingStore::new(counting, CacheConfig::disabled());
        store.put("k", b"v".to_vec()).unwrap();

        assert_eq!(store.get("k").unwrap(), b"v");
        assert_eq!(store.get("k").unwrap(), b"v");
        // Every read goes to the backend when the cache is off.
        assert_eq!(gets.load(Ordering::SeqCst), 2);
        let s = store.stats();
        assert_eq!(s.hits, 0);
        assert_eq!(s.entries, 0);
        assert!(!store.is_enabled());
    }

    // ---- correctness: no stale reads -------------------------------------

    #[test]
    fn put_invalidates_cached_key() {
        let mut store =
            CachingStore::new(MemoryStore::new(), CacheConfig::with_memory_budget(1 << 20));
        store.put("k", b"old".to_vec()).unwrap();
        assert_eq!(store.get("k").unwrap(), b"old"); // caches "old"
        store.put("k", b"new".to_vec()).unwrap(); // must invalidate
        assert_eq!(store.get("k").unwrap(), b"new"); // never serves "old"
    }

    #[test]
    fn delete_invalidates_cached_key() {
        let mut store =
            CachingStore::new(MemoryStore::new(), CacheConfig::with_memory_budget(1 << 20));
        store.put("k", b"v".to_vec()).unwrap();
        assert_eq!(store.get("k").unwrap(), b"v"); // caches
        store.delete("k").unwrap();
        assert!(matches!(store.get("k"), Err(StoreError::NotFound(_))));
    }

    #[test]
    fn explicit_invalidate_drops_one_key() {
        let mut store =
            CachingStore::new(MemoryStore::new(), CacheConfig::with_memory_budget(1 << 20));
        store.put("a", b"1".to_vec()).unwrap();
        store.put("b", b"2".to_vec()).unwrap();
        store.get("a").unwrap();
        store.get("b").unwrap();
        assert_eq!(store.stats().entries, 2);
        store.invalidate("a");
        assert_eq!(store.stats().entries, 1);
    }

    #[test]
    fn invalidate_all_clears_cache() {
        let mut store =
            CachingStore::new(MemoryStore::new(), CacheConfig::with_memory_budget(1 << 20));
        store.put("a", b"1".to_vec()).unwrap();
        store.put("b", b"2".to_vec()).unwrap();
        store.get("a").unwrap();
        store.get("b").unwrap();
        store.invalidate_all();
        let s = store.stats();
        assert_eq!(s.entries, 0);
        assert_eq!(s.bytes, 0);
    }

    #[test]
    fn external_mutation_visible_after_invalidate() {
        // Simulate a commit performed outside the wrapper: share the backend,
        // mutate it directly, then invalidate — the reader must see the new value.
        let backend: Arc<Mutex<dyn ObjectStore + Send>> = Arc::new(Mutex::new(MemoryStore::new()));
        backend.lock().unwrap().put("k", b"v1".to_vec()).unwrap();
        let store = CachingStore::from_arc(
            Arc::clone(&backend),
            CacheConfig::with_memory_budget(1 << 20),
        );

        assert_eq!(store.get("k").unwrap(), b"v1"); // caches v1
        backend.lock().unwrap().put("k", b"v2".to_vec()).unwrap(); // external write
        assert_eq!(store.get("k").unwrap(), b"v1"); // still cached (correct: not invalidated yet)
        store.invalidate("k"); // commit observed → invalidate
        assert_eq!(store.get("k").unwrap(), b"v2"); // now fresh
    }

    // ---- resource-awareness / eviction -----------------------------------

    #[test]
    fn cache_never_exceeds_byte_budget() {
        // Budget = 30 bytes; insert 10 objects of 10 bytes each.
        let mut store = CachingStore::new(MemoryStore::new(), CacheConfig::with_memory_budget(30));
        for i in 0..10 {
            let key = format!("k{i}");
            store.put(&key, vec![b'x'; 10]).unwrap();
            store.get(&key).unwrap(); // populate cache
            assert!(
                store.stats().bytes <= 30,
                "cache exceeded budget: {} bytes",
                store.stats().bytes
            );
        }
        // At most 3 entries of 10 bytes fit in 30 bytes.
        assert!(store.stats().entries <= 3);
        assert!(store.stats().evictions > 0);
    }

    #[test]
    fn lru_evicts_least_recently_used() {
        // Budget fits exactly two 10-byte objects.
        let mut store = CachingStore::new(MemoryStore::new(), CacheConfig::with_memory_budget(20));
        store.put("a", vec![1; 10]).unwrap();
        store.put("b", vec![2; 10]).unwrap();
        store.get("a").unwrap(); // a, b cached
        store.get("b").unwrap();
        store.get("a").unwrap(); // touch a → b is now LRU
        store.put("c", vec![3; 10]).unwrap();
        store.get("c").unwrap(); // inserting c must evict b (the LRU)

        let gets_before = {
            // a should still be cached (a hit, no eviction of a)
            store.get("a").unwrap();
            store.stats()
        };
        // b was evicted, so reading it is a miss that repopulates.
        assert!(gets_before.evictions >= 1);
        assert_eq!(store.stats().entries, 2);
    }

    #[test]
    fn object_larger_than_budget_is_not_cached() {
        let mut store = CachingStore::new(MemoryStore::new(), CacheConfig::with_memory_budget(8));
        store.put("big", vec![0u8; 100]).unwrap();
        assert_eq!(store.get("big").unwrap().len(), 100); // still served
        assert_eq!(store.get("big").unwrap().len(), 100);
        // Never cached (it can't fit), so it never counts against the budget.
        assert_eq!(store.stats().entries, 0);
        assert!(store.stats().bytes <= 8);
    }

    #[test]
    fn max_entries_cap_is_respected() {
        let mut store = CachingStore::new(
            MemoryStore::new(),
            CacheConfig::with_memory_budget(1 << 20).with_max_entries(2),
        );
        for i in 0..5 {
            let key = format!("k{i}");
            store.put(&key, b"x".to_vec()).unwrap();
            store.get(&key).unwrap();
            assert!(store.stats().entries <= 2);
        }
    }

    // ---- get_range -------------------------------------------------------

    #[test]
    fn get_range_served_from_cached_full_object() {
        let counting = CountingStore::new();
        let ranges = Arc::clone(&counting.range_gets);
        let mut store = CachingStore::new(counting, CacheConfig::with_memory_budget(1 << 20));
        store.put("obj", b"abcdefgh".to_vec()).unwrap();
        store.get("obj").unwrap(); // cache the whole object
        let slice = store.get_range("obj", 2, 5).unwrap();
        assert_eq!(slice, b"cde");
        // Range was sliced from cache; the backend's get_range was never called.
        assert_eq!(ranges.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn get_range_miss_delegates_to_backend() {
        let counting = CountingStore::new();
        let store = {
            let mut s = CachingStore::new(counting, CacheConfig::with_memory_budget(1 << 20));
            s.put("obj", b"abcdefgh".to_vec()).unwrap();
            s
        };
        // No prior full get → not cached → range goes to backend.
        assert_eq!(store.get_range("obj", 1, 4).unwrap(), b"bcd");
        assert_eq!(store.stats().misses, 1);
    }

    #[test]
    fn get_range_out_of_bounds_on_cached_object_errors() {
        let mut store =
            CachingStore::new(MemoryStore::new(), CacheConfig::with_memory_budget(1 << 20));
        store.put("obj", b"abc".to_vec()).unwrap();
        store.get("obj").unwrap(); // cache it
        let err = store.get_range("obj", 1, 10).unwrap_err();
        assert!(matches!(err, StoreError::RangeOutOfBounds { .. }));
    }

    // ---- list passthrough ------------------------------------------------

    #[test]
    fn list_reflects_backend() {
        let mut store =
            CachingStore::new(MemoryStore::new(), CacheConfig::with_memory_budget(1 << 20));
        store.put("a/1", b"".to_vec()).unwrap();
        store.put("a/2", b"".to_vec()).unwrap();
        store.put("b/1", b"".to_vec()).unwrap();
        assert_eq!(store.list("a/").unwrap(), vec!["a/1", "a/2"]);
    }

    // ---- behaves as a dyn ObjectStore (architecturally optional) ---------

    #[test]
    fn usable_as_boxed_object_store() {
        // The same call site accepts the cache or the bare backend — proving the
        // cache is a drop-in wrapper, not an engine-wide refactor.
        fn round_trip(store: &mut dyn ObjectStore) -> Vec<u8> {
            store.put("k", b"hello".to_vec()).unwrap();
            store.get("k").unwrap()
        }
        let mut cached: Box<dyn ObjectStore> = Box::new(CachingStore::new(
            MemoryStore::new(),
            CacheConfig::with_memory_budget(1 << 20),
        ));
        let mut bare: Box<dyn ObjectStore> = Box::new(MemoryStore::new());
        assert_eq!(round_trip(cached.as_mut()), b"hello");
        assert_eq!(round_trip(bare.as_mut()), b"hello");
    }

    #[test]
    fn counting_store_range_count_helper() {
        // Exercises the test helper's range_count accessor for completeness.
        let counting = CountingStore::new();
        assert_eq!(counting.range_count(), 0);
        assert_eq!(counting.get_count(), 0);
    }
}
