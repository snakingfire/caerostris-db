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

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
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
