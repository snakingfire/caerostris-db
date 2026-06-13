//! caerostris-db — a graph database engine backed by commodity durable object
//! storage (e.g. S3).
//!
//! Named for *Caerostris darwini* (Darwin's bark spider), which spins the
//! toughest known biological material of any spider. This crate is the engine
//! core; the binary in `main.rs` is a thin entry point over it.
//!
//! This is scaffolding. Real requirements land later — the public surface here
//! exists only to exercise the toolchain (build, test, doctest, clippy, fmt).

pub mod licenses;
pub mod query;
pub mod tck;

/// The crate version, sourced from `Cargo.toml` at compile time.
///
/// ```
/// assert!(!caerostris_db::version().is_empty());
/// ```
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_reported() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
