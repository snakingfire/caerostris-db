//! Graph database engine core.
//!
//! This module is the execution heart of caerostris-db: it interprets query
//! plans produced by the [`planner`](crate::planner) against an
//! [`ObjectStore`](crate::storage::ObjectStore) backend and manages the
//! single-writer / multi-reader transaction model.
//!
//! **Status:** stub — the engine implementation lands after the storage format
//! (SPIKE-0003) and query planner (EPIC-002) are ratified. This module exists
//! so the workspace compiles and the module tree is established from T-0001
//! onwards.

/// Placeholder marker for the not-yet-implemented engine core.
///
/// Downstream crates that import `caerostris_db::engine` will get a compile
/// error if they reference any concrete type — a deliberate guard against
/// premature coupling.
#[allow(dead_code)]
pub struct Engine;
