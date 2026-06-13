//! openCypher logical query planner (rubric Cat. 4; latency anchor for Cat. 3).
//!
//! The planner turns a parsed [`Query`](crate::cypher::ast::Query) into a
//! logical [`LogicalPlan`] — a tree of [`Operator`]s the out-of-envelope
//! detector (T-0015), the index-selection pass (EPIC-005), and the executor
//! (T-0019) consume. It performs one optimisation today: **filter push-down**,
//! the selectivity-anchoring rewrite ADR-0001's latency envelope depends on
//! (push node-property `WHERE` predicates down to the scans/expands that bind
//! their variables, so the seed set is pruned before any hop).
//!
//! # Pipeline
//!
//! 1. [`lower`](lower::lower) walks the clauses left-to-right and builds a raw
//!    operator tree (scans, expands, filters, projections, limit, …).
//! 2. [`push_down_filters`](pushdown::push_down_filters) relocates each filter
//!    to the deepest legal operator.
//!
//! The single entry point is [`plan`].
//!
//! # Estimate hooks (decision 0009)
//!
//! Every row-producing operator carries an [`Estimates`] block — the
//! cardinality / byte / tail-fan-out hooks the out-of-envelope detector reads.
//! This task defines their *shape* (so the IR is the stable surface T-0015
//! plans against) and stamps the conservative [`Estimates::unknown`] sentinel;
//! wiring real manifest statistics is T-0015's job. The fan-out hook is a
//! **tail** (p99/max) degree, never a mean, per decision 0009.

pub mod error;
pub mod lower;
pub mod plan;
pub mod pushdown;

#[cfg(test)]
mod tests;

pub use error::{PlanError, PlanResult};
pub use plan::{Estimates, HAS_LABELS_FN, LogicalPlan, Operator, ProjectionColumn, SortKey};
pub use pushdown::push_down_filters;

use crate::cypher::ast::Query;

/// Plan a parsed read-query AST into a logical [`LogicalPlan`] with filter
/// push-down applied.
///
/// # Errors
///
/// Returns a [`PlanError`] if the query is empty
/// ([`EmptyQuery`](PlanError::EmptyQuery)), uses a shape not yet lowered
/// ([`Unsupported`](PlanError::Unsupported): multi-pattern `MATCH`,
/// variable-length relationships, or an inline property map on an unnamed
/// relationship — rejected explicitly, never silently mis-planned), or
/// references an unbound expand source ([`UnboundVariable`](PlanError::UnboundVariable)).
///
/// # Examples
///
/// ```
/// use caerostris_db::cypher::parse;
/// use caerostris_db::planner::plan;
///
/// let ast = parse("MATCH (n:Person) WHERE n.age > 21 RETURN n LIMIT 10")
///     .expect("valid query");
/// let logical = plan(&ast).expect("plannable");
/// // The WHERE predicate is pushed down to sit directly on the label scan.
/// let explain = logical.explain();
/// assert!(explain.contains("LabelScan"));
/// assert!(explain.contains("Limit"));
/// ```
pub fn plan(query: &Query) -> PlanResult<LogicalPlan> {
    lower::lower(query)
}
