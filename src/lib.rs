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
//! | [`cli`] | Shared command-line dispatch for the `caerostris-db` / `caero` binaries |
//! | [`cypher`] | openCypher front-end: lexer + parser → typed AST (Cat. 4) |
//! | [`dataset`] | License-clean synthetic graph dataset generator (Cat. 10) |
//! | [`demo`] | Minimal in-memory store + `MATCH ... RETURN` executor for the end-to-end demo |
//! | [`engine`] | Graph engine core (stub — lands after SPIKE-0003) |
//! | [`index`] | Pluggable secondary-index interface (Cat. 5) |
//! | [`licenses`] | License-manifest hygiene (Cat. 12) |
//! | [`model`] | Logical property-graph data model (Node, Edge, PropertyValue, Schema) |
//! | [`planner`] | openCypher query planner (stub — lands in EPIC-002) |
//! | [`query`] | Query execution surfaces (side-effect accounting) |
//! | [`storage`] | Object-store abstraction + in-memory backend |
//! | [`tck`] | openCypher TCK pass-rate contract (Cat. 4) |
//! | [`txn`] | Transaction management (stub — lands after SPIKE-0001) |

pub mod cli;
pub mod cypher;
pub mod dataset;
pub mod demo;
pub mod engine;
pub mod index;
pub mod licenses;
pub mod model;
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
