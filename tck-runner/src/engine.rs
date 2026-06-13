//! The engine adapter the TCK harness drives.
//!
//! The harness is deliberately decoupled from the caerostris-db engine through
//! the [`Engine`] trait: language implementors (EPIC-002) plug a real engine in
//! without touching the harness. Until then, [`PendingEngine`] reports every
//! query as unsupported, so every executable scenario is counted `pending`
//! (not `fail`) — see board item `T-0002`.

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
    /// `columns` is the ordered column name list; `rows` the result rows.
    Rows {
        columns: Vec<String>,
        rows: Vec<ResultRow>,
    },
    /// The engine rejected the query with an error of the named kind
    /// (e.g. `"SyntaxError"`, `"TypeError"`) at the given phase.
    Raised { kind: String, phase: ErrorPhase },
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
