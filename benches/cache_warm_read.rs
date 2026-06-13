//! Warm-query micro-benchmark for the optional cache (T-0033, Cat. 9).
//!
//! Demonstrates that a repeated read is measurably faster with the cache **on**
//! than with it **off**, against a backend that emulates object-storage
//! round-trip latency. Run with:
//!
//! ```text
//! cargo bench --bench cache_warm_read
//! ```
//!
//! This is a dependency-free harness (`harness = false`) rather than a criterion
//! bench on purpose: pulling criterion into the core crate would drag a large
//! transitive dependency tree through the license manifest (Cat. 12) for a
//! single micro-measurement. The headline SLA benchmark (T-0016 / T-0034) is a
//! separate, heavier artifact. The same warm-vs-cold comparison is *also*
//! asserted as a regular test in `tests/cache_integration.rs`, so the property
//! is guarded by CI even though this bench is opt-in.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use caerostris_db::storage::{CacheConfig, CachingStore, MemoryStore, ObjectStore, StoreError};

/// Backend that emulates a per-read object-storage round-trip via a fixed sleep.
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

/// Time `iters` repeated reads of one hot key through `store`.
fn time_repeated_reads(store: &dyn ObjectStore, key: &str, iters: u32) -> Duration {
    let start = Instant::now();
    for _ in 0..iters {
        let v = store.get(key).expect("read");
        std::hint::black_box(&v);
    }
    start.elapsed()
}

fn main() {
    const DELAY: Duration = Duration::from_micros(500);
    const ITERS: u32 = 200;
    let payload = vec![0xCDu8; 4096];

    // Cache OFF: every read pays the backend latency.
    let mut off = CachingStore::new(LatencyStore::new(DELAY), CacheConfig::disabled());
    off.put("hot", payload.clone()).expect("put");
    let off_elapsed = time_repeated_reads(&off, "hot", ITERS);

    // Cache ON: only the first read pays the backend latency; the rest are hits.
    let mut on = CachingStore::new(
        LatencyStore::new(DELAY),
        CacheConfig::with_memory_budget(1 << 20),
    );
    on.put("hot", payload).expect("put");
    let on_elapsed = time_repeated_reads(&on, "hot", ITERS);

    let off_per = off_elapsed / ITERS;
    let on_per = on_elapsed / ITERS;
    let speedup = off_elapsed.as_secs_f64() / on_elapsed.as_secs_f64().max(f64::MIN_POSITIVE);

    println!("cache warm-read micro-benchmark (T-0033)");
    println!("  backend per-read latency : {DELAY:?}");
    println!("  iterations               : {ITERS}");
    println!("  cache OFF : total {off_elapsed:?}  ({off_per:?}/read)");
    println!("  cache ON  : total {on_elapsed:?}  ({on_per:?}/read)");
    println!("  speedup (off/on)         : {speedup:.1}x");
    println!(
        "  cache ON hits/misses     : {} / {}",
        on.stats().hits,
        on.stats().misses
    );

    assert!(
        on_elapsed < off_elapsed,
        "expected warm (cache-on) reads to be faster than cold (cache-off)"
    );
}
