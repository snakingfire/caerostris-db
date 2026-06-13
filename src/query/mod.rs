//! Query execution surfaces.
//!
//! For now this exposes only [`QueryStatistics`], the side-effect accounting the
//! openCypher TCK adapter reads to assert `Then the side effects should be:`
//! steps (BUG-0006). The executor, planner, and runtime land in EPIC-002 and
//! will record into this surface as they apply a statement.

pub mod stats;

pub use stats::{QueryStatistics, SideEffectParseError};
