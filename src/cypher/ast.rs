//! The typed openCypher abstract syntax tree (read clauses).
//!
//! This is the output of the [`parser`](crate::cypher::parser): a structured,
//! strongly-typed representation of a parsed query that the planner consumes. It
//! covers the **read** surface of openCypher — `MATCH` / `OPTIONAL MATCH`,
//! `WHERE`, `RETURN`, `WITH`, `UNWIND`, `ORDER BY`, `SKIP`, `LIMIT`, and pattern
//! syntax (nodes, directed/typed relationships, and variable-length stubs).
//!
//! Write clauses (`CREATE` / `MERGE` / `SET` / `DELETE` / `REMOVE`) are a
//! deliberate follow-up (board item T-0021); see
//! `.project/decisions/0018-cypher-front-end-scope.md` for the scope rationale.

use crate::model::PropertyValue;

/// A complete parsed query: an ordered sequence of clauses.
///
/// openCypher queries are a pipeline of clauses (`MATCH ... WITH ... RETURN ...`).
/// The parser preserves their order; the planner reads them left to right.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    /// The clauses in source order.
    pub clauses: Vec<Clause>,
}

/// A top-level query clause.
#[derive(Debug, Clone, PartialEq)]
pub enum Clause {
    /// `MATCH <pattern> [WHERE <expr>]` or `OPTIONAL MATCH ...`.
    Match(MatchClause),
    /// `UNWIND <expr> AS <var>`.
    Unwind(UnwindClause),
    /// `WITH <projection> [ORDER BY ...] [SKIP ...] [LIMIT ...] [WHERE ...]`.
    With(ProjectionClause),
    /// `RETURN <projection> [ORDER BY ...] [SKIP ...] [LIMIT ...]`.
    Return(ProjectionClause),
}

/// A `MATCH` / `OPTIONAL MATCH` clause.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchClause {
    /// `true` for `OPTIONAL MATCH`.
    pub optional: bool,
    /// One or more comma-separated path patterns.
    pub patterns: Vec<PathPattern>,
    /// An optional inline `WHERE` predicate.
    pub where_clause: Option<Expr>,
}

/// An `UNWIND <expr> AS <var>` clause.
#[derive(Debug, Clone, PartialEq)]
pub struct UnwindClause {
    /// The list-valued expression being unwound.
    pub expr: Expr,
    /// The variable bound to each element.
    pub variable: String,
}

/// A `WITH` or `RETURN` projection clause (they share a shape; only `WITH` may
/// carry a trailing `WHERE`, enforced by the parser).
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionClause {
    /// `true` if `DISTINCT` was present.
    pub distinct: bool,
    /// The projection items, or [`ReturnBody::All`] for `RETURN *` / `WITH *`.
    pub body: ReturnBody,
    /// Optional `ORDER BY` sort keys.
    pub order_by: Vec<SortItem>,
    /// Optional `SKIP <expr>`.
    pub skip: Option<Expr>,
    /// Optional `LIMIT <expr>`.
    pub limit: Option<Expr>,
    /// Optional trailing `WHERE` (legal only after `WITH`).
    pub where_clause: Option<Expr>,
}

/// The body of a projection: either an explicit item list or the `*` wildcard.
#[derive(Debug, Clone, PartialEq)]
pub enum ReturnBody {
    /// `RETURN *` / `WITH *` — project all in-scope variables.
    All {
        /// Additional explicit items after the `*` (e.g. `RETURN *, x + 1 AS y`).
        extra: Vec<ProjectionItem>,
    },
    /// An explicit list of projection items.
    Items(Vec<ProjectionItem>),
}

/// One projected expression, optionally aliased (`expr AS alias`).
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionItem {
    /// The projected expression.
    pub expr: Expr,
    /// The output column name, if `AS <alias>` was given.
    pub alias: Option<String>,
}

/// An `ORDER BY` sort key.
#[derive(Debug, Clone, PartialEq)]
pub struct SortItem {
    /// The expression to sort by.
    pub expr: Expr,
    /// Sort direction.
    pub descending: bool,
}

// --- patterns ---------------------------------------------------------------

/// A path pattern: a node, then zero or more `(relationship, node)` steps. An
/// optional leading `var =` binds the whole path to a variable
/// (`MATCH p = (a)-->(b)`).
#[derive(Debug, Clone, PartialEq)]
pub struct PathPattern {
    /// The path variable, if the pattern was written `p = ...`.
    pub path_variable: Option<String>,
    /// The starting node.
    pub start: NodePattern,
    /// The chained `(relationship, node)` steps.
    pub steps: Vec<PatternStep>,
}

/// A `(relationship)(node)` step in a path pattern.
#[derive(Debug, Clone, PartialEq)]
pub struct PatternStep {
    /// The relationship traversed.
    pub relationship: RelPattern,
    /// The node reached.
    pub node: NodePattern,
}

/// A node pattern `(var:Label {props})`. All parts are optional: `()` is a valid
/// anonymous node.
#[derive(Debug, Clone, PartialEq)]
pub struct NodePattern {
    /// The bound variable, if named.
    pub variable: Option<String>,
    /// Zero or more labels (`:A:B`).
    pub labels: Vec<String>,
    /// Inline property map `{k: v, ...}`, if present.
    pub properties: Option<Vec<(String, Expr)>>,
}

/// The direction of a relationship pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// `-->` left-to-right.
    Outgoing,
    /// `<--` right-to-left.
    Incoming,
    /// `--` undirected.
    Undirected,
}

/// A relationship pattern `-[var:TYPE*1..3 {props}]->`. The `*` length bound is
/// captured as a [`VarLength`] stub for the planner; full var-length expansion is
/// a later task.
#[derive(Debug, Clone, PartialEq)]
pub struct RelPattern {
    /// Traversal direction.
    pub direction: Direction,
    /// The bound variable, if named.
    pub variable: Option<String>,
    /// Candidate relationship types (`:A|B`); empty means any type.
    pub types: Vec<String>,
    /// Variable-length bounds, if a `*` was present.
    pub var_length: Option<VarLength>,
    /// Inline property map, if present.
    pub properties: Option<Vec<(String, Expr)>>,
}

/// A variable-length relationship bound `*`, `*2`, `*1..`, `*..5`, `*1..3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VarLength {
    /// Minimum hops (`None` ⇒ unbounded below, defaulting to 1 at plan time).
    pub min: Option<u64>,
    /// Maximum hops (`None` ⇒ unbounded above).
    pub max: Option<u64>,
}

// --- expressions ------------------------------------------------------------

/// An openCypher expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A literal value (`1`, `'x'`, `true`, `null`, `[..]`, `{..}`).
    Literal(PropertyValue),
    /// A list literal whose elements may be arbitrary expressions
    /// (`[a, b+1]`), distinct from a constant [`Expr::Literal`] list.
    List(Vec<Expr>),
    /// A map literal whose values may be arbitrary expressions.
    Map(Vec<(String, Expr)>),
    /// A variable reference.
    Variable(String),
    /// A parameter reference `$name`.
    Parameter(String),
    /// Property access `expr.key`.
    Property {
        /// The base expression.
        base: Box<Expr>,
        /// The property key.
        key: String,
    },
    /// Indexing `expr[index]`.
    Index {
        /// The base expression.
        base: Box<Expr>,
        /// The index expression.
        index: Box<Expr>,
    },
    /// A unary operation (`-x`, `NOT x`).
    Unary {
        /// The operator.
        op: UnaryOp,
        /// The operand.
        operand: Box<Expr>,
    },
    /// A binary operation (`a + b`, `a AND b`, `a STARTS WITH b`).
    Binary {
        /// The operator.
        op: BinaryOp,
        /// Left operand.
        lhs: Box<Expr>,
        /// Right operand.
        rhs: Box<Expr>,
    },
    /// `expr IS NULL` / `expr IS NOT NULL`.
    IsNull {
        /// The tested operand.
        operand: Box<Expr>,
        /// `true` for `IS NOT NULL`.
        negated: bool,
    },
    /// A function call `fn(args)`, with an optional `DISTINCT` (`count(DISTINCT x)`).
    FunctionCall {
        /// The function name (case-preserved).
        name: String,
        /// `true` if `DISTINCT` preceded the arguments.
        distinct: bool,
        /// The argument expressions.
        args: Vec<Expr>,
    },
    /// `count(*)`.
    CountStar,
}

/// A unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// Arithmetic negation `-x`.
    Negate,
    /// Logical negation `NOT x`.
    Not,
}

/// A binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    /// `+`
    Add,
    /// `-`
    Subtract,
    /// `*`
    Multiply,
    /// `/`
    Divide,
    /// `%`
    Modulo,
    /// `^`
    Power,
    /// `=`
    Equal,
    /// `<>`
    NotEqual,
    /// `<`
    LessThan,
    /// `<=`
    LessThanOrEqual,
    /// `>`
    GreaterThan,
    /// `>=`
    GreaterThanOrEqual,
    /// `AND`
    And,
    /// `OR`
    Or,
    /// `XOR`
    Xor,
    /// `IN`
    In,
    /// `STARTS WITH`
    StartsWith,
    /// `ENDS WITH`
    EndsWith,
    /// `CONTAINS`
    Contains,
}
