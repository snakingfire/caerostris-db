//! Object-storage abstraction layer.
//!
//! This module defines the [`ObjectStore`] trait — the single interface every
//! storage backend (in-memory mock, local MinIO, production S3) must satisfy.
//! All higher layers of the engine talk to a `&dyn ObjectStore`; the concrete
//! backend is injected at startup or in tests.
//!
//! ## Design goals
//!
//! - **Swappable backends.** Unit tests use [`MemoryStore`]; integration tests
//!   and production use an S3-compatible client. The same code path covers both.
//! - **Byte-oriented.** The trait deals in raw `Vec<u8>` / byte slices. The
//!   storage format layer (to be designed in SPIKE-0003) sits above this and
//!   handles serialisation.
//! - **No async at the trait level (for now).** Introducing `async_trait` or
//!   RPITIT stabilisation requirements is deferred until the engine actually
//!   needs concurrent I/O on the hot path. The trait is synchronous and
//!   object-safe. Wrappers that add async can be layered on top.
//! - **Optional layering.** Because the trait is the single interface, optional
//!   accelerators are layered *on top of* it as wrappers that themselves
//!   implement [`ObjectStore`]. [`CachingStore`](cache::CachingStore) is one such
//!   wrapper: a resource-aware read cache that is off by default and disabled by
//!   a single config flag, never required for correctness or the cold-start SLA.
//! - **Edge adjacency.** [`AdjacencyShardWriter`] and [`AdjacencyShardReader`]
//!   implement the CSR adjacency-list format from ADR 0008, supporting early-abort
//!   range reads for hop expansion within the byte-budget envelope.

pub mod adjacency;
pub mod cache;
pub mod manifest;
pub mod memory;

pub use adjacency::{
    AdjacencyShardReader, AdjacencyShardWriter, Direction, ExpandCap, Expansion, Neighbor,
    StorageFormatError,
};
pub use cache::{CacheConfig, CacheStats, CachingStore, DiskCacheConfig, EvictionPolicy};
pub use memory::MemoryStore;

/// Errors returned by [`ObjectStore`] operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// The requested key does not exist in the store.
    NotFound(String),
    /// The requested byte range is out of bounds for the stored object.
    RangeOutOfBounds {
        /// Key that was accessed.
        key: String,
        /// Length of the object in bytes.
        object_len: usize,
        /// Start of the requested range.
        start: usize,
        /// End of the requested range (exclusive).
        end: usize,
    },
    /// A backend-specific error (e.g. network failure, permission denied).
    Backend(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::NotFound(key) => write!(f, "object not found: {key:?}"),
            StoreError::RangeOutOfBounds {
                key,
                object_len,
                start,
                end,
            } => write!(
                f,
                "range {start}..{end} is out of bounds for object {key:?} \
                 (length {object_len})"
            ),
            StoreError::Backend(msg) => write!(f, "backend error: {msg}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Minimal object-store abstraction.
///
/// Every storage backend — in-memory, local-mock, production S3 — implements
/// this trait. Higher engine layers depend only on `dyn ObjectStore` so the
/// backend can be injected or swapped without recompiling the engine.
///
/// Keys are arbitrary UTF-8 strings. The store treats them as opaque; the
/// storage format layer (SPIKE-0003) is responsible for naming conventions.
///
/// # Object safety
///
/// This trait is object-safe: all methods take `&self` or `&mut self` and use
/// only sized types. Use `Arc<dyn ObjectStore>` to share across threads.
pub trait ObjectStore {
    /// Store `bytes` at `key`, creating or overwriting the object.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Backend`] if the underlying backend fails.
    fn put(&mut self, key: &str, bytes: Vec<u8>) -> Result<(), StoreError>;

    /// Retrieve all bytes stored at `key`.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::NotFound`] if the key does not exist.
    fn get(&self, key: &str) -> Result<Vec<u8>, StoreError>;

    /// Retrieve a contiguous sub-slice of the object at `key`.
    ///
    /// `start` (inclusive) and `end` (exclusive) are byte offsets into the
    /// stored object. Both must be within `[0, object_len]` and `start <= end`.
    ///
    /// # Errors
    ///
    /// - [`StoreError::NotFound`] if the key does not exist.
    /// - [`StoreError::RangeOutOfBounds`] if the range extends past the object.
    fn get_range(&self, key: &str, start: usize, end: usize) -> Result<Vec<u8>, StoreError>;

    /// Delete the object at `key`.
    ///
    /// A no-op if the key does not exist (idempotent delete).
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Backend`] if the underlying backend fails.
    fn delete(&mut self, key: &str) -> Result<(), StoreError>;

    /// List all keys whose names begin with `prefix`.
    ///
    /// Returns keys in lexicographic order. An empty `prefix` lists all keys.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Backend`] if the underlying backend fails.
    fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a store via the trait object pointer.
    fn boxed_store() -> Box<dyn ObjectStore> {
        Box::new(MemoryStore::new())
    }

    #[test]
    fn put_and_get_round_trip() {
        let mut store = boxed_store();
        let data = b"hello world".to_vec();
        store.put("foo/bar", data.clone()).unwrap();
        let got = store.get("foo/bar").unwrap();
        assert_eq!(got, data);
    }

    #[test]
    fn get_missing_key_returns_not_found() {
        let store = boxed_store();
        let err = store.get("missing").unwrap_err();
        assert!(matches!(err, StoreError::NotFound(_)));
    }

    #[test]
    fn delete_removes_key() {
        let mut store = boxed_store();
        store.put("k", b"v".to_vec()).unwrap();
        store.delete("k").unwrap();
        assert!(matches!(store.get("k"), Err(StoreError::NotFound(_))));
    }

    #[test]
    fn delete_missing_is_idempotent() {
        let mut store = boxed_store();
        // Should not return an error.
        store.delete("nonexistent").unwrap();
    }

    #[test]
    fn list_returns_prefix_sorted() {
        let mut store = boxed_store();
        store.put("a/1", b"".to_vec()).unwrap();
        store.put("a/2", b"".to_vec()).unwrap();
        store.put("b/1", b"".to_vec()).unwrap();
        let keys = store.list("a/").unwrap();
        assert_eq!(keys, vec!["a/1", "a/2"]);
    }

    #[test]
    fn list_empty_prefix_returns_all_sorted() {
        let mut store = boxed_store();
        store.put("z", b"".to_vec()).unwrap();
        store.put("a", b"".to_vec()).unwrap();
        let keys = store.list("").unwrap();
        assert_eq!(keys, vec!["a", "z"]);
    }

    #[test]
    fn get_range_returns_sub_slice() {
        let mut store = boxed_store();
        store.put("obj", b"abcdefgh".to_vec()).unwrap();
        let slice = store.get_range("obj", 2, 5).unwrap();
        assert_eq!(slice, b"cde");
    }

    #[test]
    fn get_range_out_of_bounds_returns_error() {
        let mut store = boxed_store();
        store.put("obj", b"abc".to_vec()).unwrap();
        let err = store.get_range("obj", 1, 10).unwrap_err();
        assert!(matches!(err, StoreError::RangeOutOfBounds { .. }));
    }

    #[test]
    fn get_range_full_returns_all_bytes() {
        let mut store = boxed_store();
        store.put("obj", b"hello".to_vec()).unwrap();
        let len = store.get("obj").unwrap().len();
        let got = store.get_range("obj", 0, len).unwrap();
        assert_eq!(got, b"hello");
    }

    #[test]
    fn overwrite_replaces_previous_value() {
        let mut store = boxed_store();
        store.put("k", b"old".to_vec()).unwrap();
        store.put("k", b"new".to_vec()).unwrap();
        assert_eq!(store.get("k").unwrap(), b"new");
    }
}
