//! openCypher query planner.
//!
//! The planner takes a parsed openCypher AST and produces a physical plan that
//! the [`engine`](crate::engine) executes against the object store.
//!
//! **Status:** stub — the planner lands in EPIC-002 once the TCK harness
//! (T-0002) and formal model (SPIKE-0002) are established. This module exists
//! so the workspace compiles and the module tree is established from T-0001
//! onwards.

/// Placeholder marker for the not-yet-implemented query planner.
#[allow(dead_code)]
pub struct Planner;
