//! Integration tests for the optional resource-aware cache (T-0033, Cat. 9).
//!
//! These exercise [`CachingStore`] through the crate's *public* API, the same
//! way the engine and embedded callers will. The cache wraps the
//! [`ObjectStore`] trait, so it is backend-agnostic: today the tests run over
//! the in-memory backend and a latency-injecting backend that emulates the
//! per-request round-trip cost of object storage (the exact regime in which a
//! warm cache pays off). When an S3-backed `ObjectStore` adapter lands
//! (EPIC-001), the *same* `CachingStore` wraps it unchanged — that backend
//! substitutability is the whole point of layering the cache on the trait.
//!
//! The shared local S3 mock is provisioned by `scripts/env/up.sh` +
//! `scripts/env/bucket.sh <ID>` (per `CLAUDE.md`); these trait-level tests do
//! not require it to be running, so they are part of the default suite.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use caerostris_db::storage::{CacheConfig, CachingStore, MemoryStore, ObjectStore, StoreError};

/// A backend that sleeps for a fixed delay on every read, emulating the
/// round-trip latency of object storage, and counts the reads it serves.
struct LatencyStore {
    inner: MemoryStore,
    delay: Duration,
    reads: Arc<AtomicU64>,
}

impl LatencyStore {
    fn new(delay: Duration) -> Self {
        Self {
            inner: MemoryStore::new(),
            delay,
            reads: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl ObjectStore for LatencyStore {
    fn put(&mut self, key: &str, bytes: Vec<u8>) -> Result<(), StoreError> {
        self.inner.put(key, bytes)
    }
    fn get(&self, key: &str) -> Result<Vec<u8>, StoreError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(self.delay);
        self.inner.get(key)
    }
    fn get_range(&self, key: &str, start: usize, end: usize) -> Result<Vec<u8>, StoreError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(self.delay);
        self.inner.get_range(key, start, end)
    }
    fn delete(&mut self, key: &str) -> Result<(), StoreError> {
        self.inner.delete(key)
    }
    fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        self.inner.list(prefix)
    }
}

#[test]
fn round_trip_through_cache_matches_backend() {
    let mut store = CachingStore::new(MemoryStore::new(), CacheConfig::with_memory_budget(1 << 20));
    let payload = vec![0xABu8; 4096];
    store.put("graph/segment-0", payload.clone()).unwrap();
    assert_eq!(store.get("graph/segment-0").unwrap(), payload);
    // Range slice over a cached object matches the backend's semantics.
    assert_eq!(
        store.get_range("graph/segment-0", 0, 16).unwrap(),
        &payload[0..16]
    );
    store.delete("graph/segment-0").unwrap();
    assert!(matches!(
        store.get("graph/segment-0"),
        Err(StoreError::NotFound(_))
    ));
}

#[test]
fn warm_read_is_measurably_faster_than_cold() {
    // A 5 ms per-read backend latency makes the cache's benefit observable
    // without making the test slow. The first (cold) read pays the latency;
    // the second (warm) read is served from memory and must be markedly faster.
    let backend = LatencyStore::new(Duration::from_millis(5));
    let reads = Arc::clone(&backend.reads);
    let mut store = CachingStore::new(backend, CacheConfig::with_memory_budget(1 << 20));
    store.put("hot", vec![7u8; 1024]).unwrap();

    let t0 = std::time::Instant::now();
    let cold = store.get("hot").unwrap();
    let cold_elapsed = t0.elapsed();

    let t1 = std::time::Instant::now();
    let warm = store.get("hot").unwrap();
    let warm_elapsed = t1.elapsed();

    assert_eq!(cold, warm);
    // The backend was hit exactly once (cold); the warm read never touched it.
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    // Warm must be faster than cold by a clear margin (cold pays >=5 ms).
    assert!(
        warm_elapsed * 2 < cold_elapsed,
        "expected warm read ({warm_elapsed:?}) to be much faster than cold ({cold_elapsed:?})"
    );
    assert_eq!(store.stats().hits, 1);
}

#[test]
fn disabled_cache_always_hits_backend() {
    // With the cache off, every read pays the backend cost — proving the cache
    // is genuinely optional and not silently load-bearing.
    let backend = LatencyStore::new(Duration::from_millis(1));
    let reads = Arc::clone(&backend.reads);
    let mut store = CachingStore::new(backend, CacheConfig::disabled());
    store.put("k", b"v".to_vec()).unwrap();

    for _ in 0..5 {
        assert_eq!(store.get("k").unwrap(), b"v");
    }
    assert_eq!(reads.load(Ordering::SeqCst), 5);
    assert_eq!(store.stats().hits, 0);
    assert_eq!(store.stats().entries, 0);
}

#[test]
fn tight_budget_evicts_and_stays_bounded_under_load() {
    // Stream many distinct objects through a deliberately tiny budget; the
    // resident set must never exceed the budget, proving no unbounded growth.
    const BUDGET: usize = 4 * 1024; // 4 KiB
    const OBJ: usize = 1024; // 1 KiB each
    let mut store = CachingStore::new(MemoryStore::new(), CacheConfig::with_memory_budget(BUDGET));

    for i in 0..1000 {
        let key = format!("obj/{i:04}");
        store.put(&key, vec![(i % 256) as u8; OBJ]).unwrap();
        let got = store.get(&key).unwrap();
        assert_eq!(got.len(), OBJ);
        assert!(
            store.stats().bytes <= BUDGET,
            "cache exceeded {BUDGET} byte budget: {} bytes at i={i}",
            store.stats().bytes
        );
    }
    // At most BUDGET/OBJ = 4 objects resident; eviction did real work.
    assert!(store.stats().entries <= BUDGET / OBJ);
    assert!(store.stats().evictions > 0);
}

#[test]
fn no_stale_read_after_overwrite_commit() {
    // The correctness invariant: a reader never sees a value the backend has
    // since overwritten through the same wrapper.
    let mut store = CachingStore::new(MemoryStore::new(), CacheConfig::with_memory_budget(1 << 20));
    store.put("manifest", b"v1".to_vec()).unwrap();
    assert_eq!(store.get("manifest").unwrap(), b"v1"); // cache v1
    store.put("manifest", b"v2".to_vec()).unwrap(); // commit v2 (invalidates)
    assert_eq!(store.get("manifest").unwrap(), b"v2");
}

#[test]
fn shared_backend_invalidate_propagates() {
    // Two wrappers share one backend (single-writer / multi-reader). A write
    // via the writer wrapper, followed by an invalidate on the reader wrapper,
    // makes the new value visible to the reader.
    let backend: Arc<std::sync::Mutex<dyn ObjectStore + Send>> =
        Arc::new(std::sync::Mutex::new(MemoryStore::new()));
    backend.lock().unwrap().put("k", b"v1".to_vec()).unwrap();

    let reader = CachingStore::from_arc(
        Arc::clone(&backend),
        CacheConfig::with_memory_budget(1 << 20),
    );
    assert_eq!(reader.get("k").unwrap(), b"v1"); // reader caches v1

    backend.lock().unwrap().put("k", b"v2".to_vec()).unwrap(); // writer commits v2
    reader.invalidate("k"); // reader observes the new manifest version
    assert_eq!(reader.get("k").unwrap(), b"v2");
}

// ---- BUG-0017: lost-invalidation race on the miss-populate path -----------
//
// A cold read misses the cache, releases the cache lock, and fetches from the
// backend. In the window before it re-acquires the lock to populate, a
// committing writer advances the backend and calls invalidate / invalidate_all.
// Without the generation fence the reader caches the pre-commit bytes and every
// subsequent read serves the stale version forever. These tests reproduce that
// window **deterministically** (no `loom`): the backend's `get()` fires the
// racing commit+invalidate at exactly the populate window, on the same
// Arc-shared cache the reader is populating.

/// Backend over a **shared** [`MemoryStore`] whose `get()` runs a one-shot hook
/// after reading the bytes but before returning them — opening the post-fetch /
/// pre-populate window. The shared store is the same one the commit hook writes
/// to, so the post-commit version is visible to subsequent reads (which is what
/// makes a poisoned cache observable as a stale read).
struct WindowInjectingStore {
    inner: Arc<Mutex<MemoryStore>>,
    hook: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl WindowInjectingStore {
    fn new(inner: Arc<Mutex<MemoryStore>>, hook: Box<dyn FnOnce() + Send>) -> Self {
        Self {
            inner,
            hook: Mutex::new(Some(hook)),
        }
    }
}

impl ObjectStore for WindowInjectingStore {
    fn put(&mut self, key: &str, bytes: Vec<u8>) -> Result<(), StoreError> {
        self.inner.lock().unwrap().put(key, bytes)
    }
    fn get(&self, key: &str) -> Result<Vec<u8>, StoreError> {
        // Read the version live *now* — models S3 returning the bytes that were
        // current when the GET was issued.
        let bytes = self.inner.lock().unwrap().get(key)?;
        // Fire the racing commit + invalidation once, while the cache wrapper
        // holds no cache lock.
        if let Some(hook) = self.hook.lock().unwrap().take() {
            hook();
        }
        Ok(bytes)
    }
    fn get_range(&self, key: &str, start: usize, end: usize) -> Result<Vec<u8>, StoreError> {
        self.inner.lock().unwrap().get_range(key, start, end)
    }
    fn delete(&mut self, key: &str) -> Result<(), StoreError> {
        self.inner.lock().unwrap().delete(key)
    }
    fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        self.inner.lock().unwrap().list(prefix)
    }
}

#[test]
fn miss_populate_raced_by_invalidate_never_serves_stale() {
    // One shared backend that both the reader and the commit hook operate on.
    let backend: Arc<Mutex<MemoryStore>> = Arc::new(Mutex::new(MemoryStore::new()));
    backend
        .lock()
        .unwrap()
        .put("manifest", b"v1".to_vec())
        .unwrap();

    // The cache is cloneable and shares its state via Arc, so the hook can
    // invalidate the SAME cache the reader is about to populate.
    let cache_slot: Arc<Mutex<Option<CachingStore>>> = Arc::new(Mutex::new(None));

    let cache_slot_for_hook = Arc::clone(&cache_slot);
    let backend_for_hook = Arc::clone(&backend);
    let hook = Box::new(move || {
        backend_for_hook
            .lock()
            .unwrap()
            .put("manifest", b"v2".to_vec())
            .unwrap();
        if let Some(cache) = cache_slot_for_hook.lock().unwrap().as_ref() {
            cache.invalidate("manifest");
        }
    });

    let reader_backend = WindowInjectingStore::new(Arc::clone(&backend), hook);
    let cache = CachingStore::new(reader_backend, CacheConfig::with_memory_budget(1 << 20));
    *cache_slot.lock().unwrap() = Some(cache.clone());

    // Cold read: fetches v1, the hook commits v2 + invalidates, the fence drops
    // v1 (generation moved). The bytes returned to THIS read are v1 (a read
    // serializable before the commit), but the cache is NOT poisoned.
    assert_eq!(
        cache.get("manifest").unwrap(),
        b"v1",
        "the racing read itself sees the pre-commit bytes"
    );
    // Decisive: the next read must reflect the committed version. With the bug
    // this returns the stale v1 forever.
    assert_eq!(
        cache.get("manifest").unwrap(),
        b"v2",
        "after commit+invalidate raced the populate, reads must reflect v2"
    );
    assert_eq!(cache.get("manifest").unwrap(), b"v2");
}

#[test]
fn miss_populate_raced_by_invalidate_all_never_serves_stale() {
    let backend: Arc<Mutex<MemoryStore>> = Arc::new(Mutex::new(MemoryStore::new()));
    backend.lock().unwrap().put("k", b"old".to_vec()).unwrap();

    let cache_slot: Arc<Mutex<Option<CachingStore>>> = Arc::new(Mutex::new(None));
    let cache_slot_for_hook = Arc::clone(&cache_slot);
    let backend_for_hook = Arc::clone(&backend);
    let hook = Box::new(move || {
        backend_for_hook
            .lock()
            .unwrap()
            .put("k", b"new".to_vec())
            .unwrap();
        if let Some(cache) = cache_slot_for_hook.lock().unwrap().as_ref() {
            cache.invalidate_all();
        }
    });

    let reader_backend = WindowInjectingStore::new(Arc::clone(&backend), hook);
    let cache = CachingStore::new(reader_backend, CacheConfig::with_memory_budget(1 << 20));
    *cache_slot.lock().unwrap() = Some(cache.clone());

    assert_eq!(cache.get("k").unwrap(), b"old");
    assert_eq!(
        cache.get("k").unwrap(),
        b"new",
        "invalidate_all racing the populate must not leave a stale entry"
    );
}

#[test]
fn unraced_miss_still_populates_and_warms() {
    // The fence must not over-fire: a cold read with no racing invalidation
    // still caches, so the second read is a hit.
    let backend: Arc<Mutex<MemoryStore>> = Arc::new(Mutex::new(MemoryStore::new()));
    backend.lock().unwrap().put("k", b"v".to_vec()).unwrap();
    let reader = CachingStore::from_arc(
        Arc::clone(&backend) as Arc<Mutex<dyn ObjectStore + Send>>,
        CacheConfig::with_memory_budget(1 << 20),
    );
    assert_eq!(reader.get("k").unwrap(), b"v"); // miss → populate
    assert_eq!(reader.get("k").unwrap(), b"v"); // hit
    let s = reader.stats();
    assert_eq!(s.hits, 1, "second read was a cache hit");
    assert_eq!(s.misses, 1, "only the first read missed");
    assert_eq!(s.entries, 1, "the unraced object was cached");
}
