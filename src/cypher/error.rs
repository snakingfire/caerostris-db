//! Structured errors for the openCypher front-end.
//!
//! Both the lexer and the parser report failures as a [`CypherError`] carrying a
//! source [`Location`] (1-based line and column, plus the byte offset) and a
//! human-readable message. Errors are **never** panics: malformed input is data,
//! and the TCK asserts `SyntaxError` outcomes that the harness must observe as a
//! structured result rather than an aborted process.

use std::fmt;

/// A position in the source text. Lines and columns are 1-based (the first
/// character is line 1, column 1); `offset` is the 0-based byte index into the
/// source string, which the caller can use to slice the offending span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    /// 1-based line number (incremented on each `\n`).
    pub line: usize,
    /// 1-based column number, counted in Unicode scalar values (`char`s), so a
    /// multi-byte character advances the column by one, not by its byte length.
    pub column: usize,
    /// 0-based byte offset into the source string.
    pub offset: usize,
}

impl Location {
    /// The start-of-input location: line 1, column 1, offset 0.
    #[must_use]
    pub const fn start() -> Self {
        Location {
            line: 1,
            column: 1,
            offset: 0,
        }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}, column {}", self.line, self.column)
    }
}

/// The phase that produced an error. The openCypher TCK distinguishes
/// compile-time (lex/parse/semantic) from runtime errors; everything this
/// front-end produces is compile-time, but the kind is preserved so callers can
/// surface the right `SyntaxError`/`SemanticError` family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CypherErrorKind {
    /// A lexing failure: an illegal character, an unterminated string, etc.
    Lex,
    /// A parsing failure: a token sequence that does not match the grammar.
    Parse,
}

/// A structured front-end error: kind + source location + message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CypherError {
    /// Which phase raised the error.
    pub kind: CypherErrorKind,
    /// Where in the source the error was detected.
    pub location: Location,
    /// A human-readable description of what went wrong.
    pub message: String,
}

impl CypherError {
    /// Construct a lexing error at `location`.
    pub(crate) fn lex(location: Location, message: impl Into<String>) -> Self {
        CypherError {
            kind: CypherErrorKind::Lex,
            location,
            message: message.into(),
        }
    }

    /// Construct a parsing error at `location`.
    pub(crate) fn parse(location: Location, message: impl Into<String>) -> Self {
        CypherError {
            kind: CypherErrorKind::Parse,
            location,
            message: message.into(),
        }
    }
}

impl fmt::Display for CypherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let phase = match self.kind {
            CypherErrorKind::Lex => "lex error",
            CypherErrorKind::Parse => "parse error",
        };
        write!(f, "{phase} at {}: {}", self.location, self.message)
    }
}

impl std::error::Error for CypherError {}

/// The result type for the openCypher front-end.
pub type CypherResult<T> = Result<T, CypherError>;
