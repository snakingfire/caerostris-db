//! caerostris-db — a graph database engine backed by commodity durable object
//! storage (e.g. S3).
//!
//! Named for *Caerostris darwini* (Darwin's bark spider), which spins the
//! toughest known biological material of any spider. This crate is the engine
//! core; the binary in `main.rs` is a thin entry point over it.
//!
//! ## Crate layout
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`engine`] | Graph engine core (stub — lands after SPIKE-0003) |
//! | [`licenses`] | License-manifest hygiene (Cat. 12) |
//! | [`planner`] | openCypher query planner (stub — lands in EPIC-002) |
//! | [`query`] | Query execution surfaces (side-effect accounting) |
//! | [`storage`] | Object-store abstraction + in-memory backend |
//! | [`tck`] | openCypher TCK pass-rate contract (Cat. 4) |
//! | [`txn`] | Transaction management (stub — lands after SPIKE-0001) |

pub mod engine;
pub mod licenses;
pub mod planner;
pub mod query;
pub mod storage;
pub mod tck;
pub mod txn;

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
