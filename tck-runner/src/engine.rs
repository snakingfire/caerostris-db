//! The engine adapter the TCK harness drives.
//!
//! The harness is deliberately decoupled from the caerostris-db engine through
//! the [`Engine`] trait: language implementors (EPIC-002) plug a real engine in
//! without touching the harness. Until then, [`PendingEngine`] reports every
//! query as unsupported, so every executable scenario is counted `pending`
//! (not `fail`) — see board item `T-0002`.
//!
//! An [`ExecOutcome::Rows`] carries the engine's [`QueryStatistics`] alongside
//! the result rows so the harness can assert `Then the side effects should be:`
//! TCK steps as real pass/fail (BUG-0006 / Decision 0012), not just the rows.

use caerostris_db::query::QueryStatistics;

/// A single row of a query result: an ordered list of column values, already
/// rendered to the TCK's canonical string form (e.g. `"1"`, `"'abc'"`,
/// `"({n: 1})"`).
pub type ResultRow = Vec<String>;

/// The outcome of asking the engine to execute one openCypher statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecOutcome {
    /// The engine does not (yet) support some construct in the query. The
    /// harness counts the owning scenario as `pending`, never `fail`.
    Unsupported,
    /// The engine executed the query and produced a (possibly empty) result.
    /// `columns` is the ordered column name list; `rows` the result rows;
    /// `side_effects` the [`QueryStatistics`] the engine recorded while applying
    /// the statement (all-zero for a read-only query). The harness reads
    /// `side_effects` to assert `Then the side effects should be:` steps.
    Rows {
        columns: Vec<String>,
        rows: Vec<ResultRow>,
        side_effects: QueryStatistics,
    },
    /// The engine rejected the query with an error of the named kind
    /// (e.g. `"SyntaxError"`, `"TypeError"`) at the given phase.
    Raised { kind: String, phase: ErrorPhase },
}

impl ExecOutcome {
    /// Construct a [`Rows`](ExecOutcome::Rows) outcome with no side effects —
    /// the common case for a read-only query. Keeps call sites that do not
    /// care about side effects terse.
    #[must_use]
    pub fn rows(columns: Vec<String>, rows: Vec<ResultRow>) -> Self {
        ExecOutcome::Rows {
            columns,
            rows,
            side_effects: QueryStatistics::new(),
        }
    }

    /// Construct a [`Rows`](ExecOutcome::Rows) outcome carrying the engine's
    /// recorded [`QueryStatistics`]. A write statement reports its side effects
    /// here; the harness asserts them against the scenario's expected table.
    #[must_use]
    pub fn rows_with_side_effects(
        columns: Vec<String>,
        rows: Vec<ResultRow>,
        side_effects: QueryStatistics,
    ) -> Self {
        ExecOutcome::Rows {
            columns,
            rows,
            side_effects,
        }
    }

    /// The side effects the engine reported for this outcome. A non-`Rows`
    /// outcome (unsupported / raised) reports no side effects — the correct
    /// "all zero" value for a statement that did not apply successfully.
    #[must_use]
    pub fn side_effects(&self) -> QueryStatistics {
        match self {
            ExecOutcome::Rows { side_effects, .. } => *side_effects,
            ExecOutcome::Unsupported | ExecOutcome::Raised { .. } => QueryStatistics::new(),
        }
    }
}

/// When an error was raised, mirroring the TCK's distinction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorPhase {
    /// `... should be raised at compile time` — parse / semantic analysis.
    CompileTime,
    /// `... should be raised at runtime` — during execution.
    Runtime,
    /// `... should be raised at any time`.
    AnyTime,
}

/// The adapter the harness drives. A real implementation opens a fresh,
/// isolated engine instance per scenario; the stub here is stateless.
pub trait Engine {
    /// Execute one openCypher statement against the current graph state.
    ///
    /// Setup statements (`Given having executed:`) and the scenario's main
    /// query both flow through here, in order, against the same instance.
    fn execute(&mut self, query: &str) -> ExecOutcome;
}

/// The default stub: every query is reported [`ExecOutcome::Unsupported`], so
/// the harness counts every executable scenario as `pending`. This keeps the
/// Cat. 4 score a real, non-zero-denominator number from day one while the
/// language engine is built out in EPIC-002.
#[derive(Debug, Default, Clone, Copy)]
pub struct PendingEngine;

impl Engine for PendingEngine {
    fn execute(&mut self, _query: &str) -> ExecOutcome {
        ExecOutcome::Unsupported
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_engine_reports_everything_unsupported() {
        let mut e = PendingEngine;
        assert_eq!(e.execute("MATCH (n) RETURN n"), ExecOutcome::Unsupported);
        assert_eq!(e.execute("CREATE (:X)"), ExecOutcome::Unsupported);
    }
}
