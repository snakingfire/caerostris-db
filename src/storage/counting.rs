//! A byte-counting [`ObjectStore`] wrapper for budget assertions.
//!
//! [`CountingStore`] wraps any [`ObjectStore`] and tallies the number of bytes
//! returned by `get` / `get_range` and the number of GET requests. It is the
//! instrument the latency selectivity-envelope work (ADR 0001 / ADR 0008 §6)
//! uses to **prove a read pattern fits the byte budget**: a test issues the
//! query's reads through the wrapper and asserts `bytes_fetched ≤ B_max`.
//!
//! It is the same mechanism that demonstrates ADR 0008 land-gate **condition
//! C3** — that a single-property filter read fetches only that column's chunk,
//! not the whole node record (`tests/ncol_columnar_read.rs`).
//!
//! The wrapper is backend-agnostic: it counts bytes at the [`ObjectStore`]
//! boundary, so the figure is identical whether the inner store is the
//! in-memory backend or a real S3 client — the bytes the wrapper reports are
//! exactly the bytes a real range-GET would transfer.

use std::cell::Cell;

use super::{ObjectStore, StoreError};

/// An [`ObjectStore`] decorator that tallies bytes fetched and GET requests.
///
/// Read counters live behind [`Cell`]s so they update through the `&self`
/// read methods of [`ObjectStore`]; the wrapper is single-threaded (a test /
/// single-reader instrument), matching the engine's single-threaded
/// [`MemoryStore`](super::MemoryStore).
///
/// ```
/// use caerostris_db::storage::{CountingStore, MemoryStore, ObjectStore};
///
/// let mut inner = MemoryStore::new();
/// inner.put("k", b"abcdef".to_vec()).unwrap();
/// let store = CountingStore::new(inner);
/// let _ = store.get_range("k", 1, 4).unwrap(); // 3 bytes
/// assert_eq!(store.bytes_fetched(), 3);
/// assert_eq!(store.get_requests(), 1);
/// ```
#[derive(Debug)]
pub struct CountingStore<S: ObjectStore> {
    inner: S,
    bytes_fetched: Cell<u64>,
    get_requests: Cell<u64>,
}

impl<S: ObjectStore> CountingStore<S> {
    /// Wrap `inner`, starting both counters at zero.
    pub fn new(inner: S) -> Self {
        CountingStore {
            inner,
            bytes_fetched: Cell::new(0),
            get_requests: Cell::new(0),
        }
    }

    /// Total bytes returned by `get` + `get_range` since construction / reset.
    #[must_use]
    pub fn bytes_fetched(&self) -> u64 {
        self.bytes_fetched.get()
    }

    /// Total `get` + `get_range` requests since construction / reset.
    #[must_use]
    pub fn get_requests(&self) -> u64 {
        self.get_requests.get()
    }

    /// Reset both counters to zero (e.g. to measure one query phase in
    /// isolation).
    pub fn reset(&self) {
        self.bytes_fetched.set(0);
        self.get_requests.set(0);
    }

    /// Borrow the wrapped store (read-only) — useful for assertions about its
    /// state that do not go through the counted read path.
    pub fn inner(&self) -> &S {
        &self.inner
    }

    fn record(&self, bytes: usize) {
        self.bytes_fetched
            .set(self.bytes_fetched.get() + bytes as u64);
        self.get_requests.set(self.get_requests.get() + 1);
    }
}

impl<S: ObjectStore> ObjectStore for CountingStore<S> {
    fn put(&mut self, key: &str, bytes: Vec<u8>) -> Result<(), StoreError> {
        // Writes are not part of the read budget; not counted.
        self.inner.put(key, bytes)
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, StoreError> {
        let v = self.inner.get(key)?;
        self.record(v.len());
        Ok(v)
    }

    fn get_range(&self, key: &str, start: usize, end: usize) -> Result<Vec<u8>, StoreError> {
        let v = self.inner.get_range(key, start, end)?;
        self.record(v.len());
        Ok(v)
    }

    fn delete(&mut self, key: &str) -> Result<(), StoreError> {
        self.inner.delete(key)
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        // LIST returns keys, not object bytes; not part of the byte budget.
        self.inner.list(prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemoryStore;

    fn store_with(key: &str, bytes: &[u8]) -> CountingStore<MemoryStore> {
        let mut inner = MemoryStore::new();
        inner.put(key, bytes.to_vec()).unwrap();
        CountingStore::new(inner)
    }

    #[test]
    fn counts_get_range_bytes() {
        let store = store_with("k", b"abcdefgh");
        let _ = store.get_range("k", 2, 5).unwrap();
        assert_eq!(store.bytes_fetched(), 3);
        assert_eq!(store.get_requests(), 1);
    }

    #[test]
    fn counts_full_get_bytes() {
        let store = store_with("k", b"hello");
        let _ = store.get("k").unwrap();
        assert_eq!(store.bytes_fetched(), 5);
        assert_eq!(store.get_requests(), 1);
    }

    #[test]
    fn accumulates_across_reads() {
        let store = store_with("k", b"0123456789");
        let _ = store.get_range("k", 0, 4).unwrap();
        let _ = store.get_range("k", 4, 10).unwrap();
        assert_eq!(store.bytes_fetched(), 10);
        assert_eq!(store.get_requests(), 2);
    }

    #[test]
    fn reset_zeroes_counters() {
        let store = store_with("k", b"abc");
        let _ = store.get("k").unwrap();
        store.reset();
        assert_eq!(store.bytes_fetched(), 0);
        assert_eq!(store.get_requests(), 0);
    }

    #[test]
    fn put_and_delete_and_list_not_counted() {
        let mut store = store_with("a", b"xx");
        store.put("b", b"yy".to_vec()).unwrap();
        let _ = store.list("").unwrap();
        store.delete("b").unwrap();
        assert_eq!(store.bytes_fetched(), 0);
        assert_eq!(store.get_requests(), 0);
    }

    #[test]
    fn failed_read_is_not_counted() {
        let store = store_with("k", b"abc");
        let _ = store.get("missing");
        let _ = store.get_range("k", 0, 99); // out of bounds
        assert_eq!(store.bytes_fetched(), 0);
        assert_eq!(store.get_requests(), 0);
    }
}
