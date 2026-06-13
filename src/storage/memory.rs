//! In-memory [`ObjectStore`] implementation for unit tests.
//!
//! [`MemoryStore`] stores objects in a `BTreeMap` and is fully deterministic,
//! thread-unsafe (single-threaded use only), and has no external dependencies.
//! It is the backend used in any unit test that does not need to exercise real
//! S3 semantics.
//!
//! For integration tests against the local MinIO mock, use the S3 adapter
//! wired by `tests/integration_helpers` (in `tests/`).

use std::collections::BTreeMap;

use super::{ObjectStore, StoreError};

/// A fully in-memory, single-threaded [`ObjectStore`].
///
/// All operations run in O(n) time or better where n is the number of stored
/// objects. Suitable for unit tests; not suitable for production.
///
/// ```
/// use caerostris_db::storage::{MemoryStore, ObjectStore};
///
/// let mut store = MemoryStore::new();
/// store.put("greet", b"hello".to_vec()).unwrap();
/// assert_eq!(store.get("greet").unwrap(), b"hello");
/// store.delete("greet").unwrap();
/// assert!(store.get("greet").is_err());
/// ```
#[derive(Debug, Default, Clone)]
pub struct MemoryStore {
    data: BTreeMap<String, Vec<u8>>,
}

impl MemoryStore {
    /// Create an empty in-memory store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of objects currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// True if the store contains no objects.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl ObjectStore for MemoryStore {
    fn put(&mut self, key: &str, bytes: Vec<u8>) -> Result<(), StoreError> {
        self.data.insert(key.to_owned(), bytes);
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, StoreError> {
        self.data
            .get(key)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(key.to_owned()))
    }

    fn get_range(&self, key: &str, start: usize, end: usize) -> Result<Vec<u8>, StoreError> {
        let data = self
            .data
            .get(key)
            .ok_or_else(|| StoreError::NotFound(key.to_owned()))?;

        let len = data.len();

        if end > len || start > end {
            return Err(StoreError::RangeOutOfBounds {
                key: key.to_owned(),
                object_len: len,
                start,
                end,
            });
        }

        Ok(data[start..end].to_vec())
    }

    fn delete(&mut self, key: &str) -> Result<(), StoreError> {
        self.data.remove(key);
        Ok(())
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        let keys = self
            .data
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        Ok(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_store_is_empty() {
        let store = MemoryStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn len_tracks_insertions() {
        let mut store = MemoryStore::new();
        store.put("a", b"1".to_vec()).unwrap();
        assert_eq!(store.len(), 1);
        store.put("b", b"2".to_vec()).unwrap();
        assert_eq!(store.len(), 2);
        store.delete("a").unwrap();
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn doctest_smoke() {
        let mut store = MemoryStore::new();
        store.put("greet", b"hello".to_vec()).unwrap();
        assert_eq!(store.get("greet").unwrap(), b"hello");
        store.delete("greet").unwrap();
        assert!(store.get("greet").is_err());
    }

    #[test]
    fn get_range_three_bytes() {
        let mut store = MemoryStore::new();
        store.put("x", b"abcdef".to_vec()).unwrap();
        // Bytes at index 1, 2, 3 (end exclusive = 4)
        let r = store.get_range("x", 1, 4).unwrap();
        assert_eq!(r, b"bcd");
    }

    #[test]
    fn get_range_empty_range_returns_empty() {
        let mut store = MemoryStore::new();
        store.put("x", b"abc".to_vec()).unwrap();
        let r = store.get_range("x", 1, 1).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn clone_is_independent() {
        let mut original = MemoryStore::new();
        original.put("k", b"v".to_vec()).unwrap();
        let mut clone = original.clone();
        clone.put("k", b"other".to_vec()).unwrap();
        assert_eq!(original.get("k").unwrap(), b"v");
        assert_eq!(clone.get("k").unwrap(), b"other");
    }
}
