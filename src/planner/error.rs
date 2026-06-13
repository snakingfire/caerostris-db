//! Planner errors.
//!
//! Lowering an AST to the logical IR can fail for reasons that are *semantic*
//! rather than syntactic (the parser already rejected malformed text): an
//! `OPTIONAL MATCH` with no preceding rows to attach to, an expand from an
//! unbound variable, an unsupported clause shape. These surface as a
//! [`PlanError`] — structured data, never a panic — so the query layer can
//! report them to the client and the TCK adapter can observe them.

use std::fmt;

/// An error raised while lowering an AST [`Query`](crate::cypher::ast::Query)
/// into the logical [`LogicalPlan`](super::plan::LogicalPlan).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// A pattern referenced a relationship/destination from a node variable that
    /// was never bound by an earlier scan or expand.
    UnboundVariable {
        /// The offending variable name.
        variable: String,
    },
    /// A clause appeared in a position the read-query planner cannot lower
    /// (e.g. a stray `WHERE`-only clause, or a write clause the front-end has
    /// not yet wired). Carries a human-readable reason.
    Unsupported {
        /// Why the clause could not be lowered.
        reason: String,
    },
    /// A query produced no operators at all (e.g. an empty clause list).
    EmptyQuery,
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanError::UnboundVariable { variable } => {
                write!(f, "variable `{variable}` is not bound at this point")
            }
            PlanError::Unsupported { reason } => {
                write!(f, "unsupported query shape: {reason}")
            }
            PlanError::EmptyQuery => write!(f, "query has no clauses to plan"),
        }
    }
}

impl std::error::Error for PlanError {}

/// The planner's `Result` alias.
pub type PlanResult<T> = Result<T, PlanError>;
