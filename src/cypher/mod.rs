//! The openCypher front-end: source text → tokens → a typed AST.
//!
//! This module is the entrance to the openCypher query pipeline (EPIC-002,
//! rubric Cat. 4). It has three layers:
//!
//! 1. [`lexer`] — turns source text into a [`token::Token`] stream, tracking a
//!    1-based line/column [`error::Location`] for every token.
//! 2. [`parser`] — a recursive-descent + precedence-climbing parser that turns
//!    the token stream into an [`ast::Query`].
//! 3. [`ast`] — the typed AST the planner consumes.
//!
//! Failures at either layer are structured [`error::CypherError`]s carrying a
//! source location and message — never panics — so the TCK harness (T-0002) can
//! observe `SyntaxError` outcomes as data.
//!
//! # Scope
//!
//! This task (T-0017) implements the **read** surface of openCypher: `MATCH` /
//! `OPTIONAL MATCH`, `WHERE`, `RETURN`, `WITH`, `UNWIND`, `ORDER BY`, `SKIP`,
//! `LIMIT`, pattern syntax (nodes, directed/typed relationships, variable-length
//! stubs), and the expression sub-language. Write clauses (`CREATE` / `MERGE` /
//! `SET` / `DELETE` / `REMOVE`) and the long tail of expression forms
//! (comprehensions, `CASE`, quantifier predicates) are deliberate follow-ups —
//! see `.project/decisions/0018-cypher-front-end-scope.md`.
//!
//! # Example
//!
//! ```
//! use caerostris_db::cypher::parse;
//!
//! let query = parse("MATCH (n:Person) WHERE n.age > 21 RETURN n.name AS name LIMIT 10")
//!     .expect("valid read query");
//! assert_eq!(query.clauses.len(), 2);
//! ```

pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod token;

pub use ast::Query;
pub use error::{CypherError, CypherErrorKind, CypherResult, Location};
pub use lexer::tokenize;
pub use parser::parse;
pub use token::{Keyword, Token, TokenKind};
