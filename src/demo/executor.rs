//! A minimal `MATCH ... RETURN` executor over the in-memory [`GraphStore`].
//!
//! This is the demo's query engine: it takes a parsed openCypher [`Query`] and
//! evaluates the read shapes the hackathon demo needs, returning bound rows. It
//! is intentionally tiny — the production planner/executor lands in EPIC-002.
//! Two pattern shapes are supported:
//!
//! 1. **Single node** — `MATCH (n:Label {key: value}) RETURN n`. The label is an
//!    optional filter (omit it to match any node); the inline property map is an
//!    optional equality filter on every listed key.
//! 2. **One hop** — `MATCH (a:Label)-[:REL]->(b) RETURN a, b`. Filters apply to
//!    both endpoints; the relationship type (`:REL`) is an optional filter.
//!
//! An optional trailing `WHERE n.key = <literal>` adds further property-equality
//! filtering. Anything outside this surface returns an [`ExecError`] rather than
//! a wrong answer — the demo would rather say "unsupported" than mislead.

use std::collections::BTreeMap;

use crate::cypher::ast::{
    BinaryOp, Clause, Direction, Expr, MatchClause, NodePattern, ProjectionClause, Query,
    ReturnBody,
};
use crate::model::{Edge, Node, PropertyValue};

use super::store::GraphStore;

/// A value bound to a `RETURN` variable: a node or an edge from the store.
#[derive(Debug, Clone, PartialEq)]
pub enum Binding {
    /// A bound node.
    Node(Node),
    /// A bound edge (relationship).
    Edge(Edge),
}

/// One result row: the projected columns, in `RETURN` order, each labelled by
/// the variable name it was returned under.
pub type Row = Vec<(String, Binding)>;

/// Why a query could not be executed.
///
/// The demo executor covers a deliberate, small slice of openCypher; anything
/// outside it is reported as an error so the demo never returns a misleading
/// answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecError {
    /// The query did not contain exactly one `MATCH` followed by one `RETURN`.
    UnsupportedShape(String),
    /// A `RETURN` item referenced a variable not bound by the pattern.
    UnknownVariable(String),
    /// A `WHERE` / property filter used a form the demo does not evaluate.
    UnsupportedExpression(String),
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecError::UnsupportedShape(m) => write!(f, "unsupported query shape: {m}"),
            ExecError::UnknownVariable(v) => write!(f, "RETURN references unknown variable '{v}'"),
            ExecError::UnsupportedExpression(m) => write!(f, "unsupported expression: {m}"),
        }
    }
}

impl std::error::Error for ExecError {}

/// Execute a parsed `MATCH ... RETURN` [`Query`] against the store.
///
/// # Errors
/// Returns [`ExecError`] if the query is outside the demo's supported surface
/// (see the module docs) or references an unbound variable.
pub fn execute(store: &GraphStore, query: &Query) -> Result<Vec<Row>, ExecError> {
    let (match_clause, projection) = split_match_return(query)?;

    if match_clause.patterns.len() != 1 {
        return Err(ExecError::UnsupportedShape(
            "exactly one path pattern is supported".into(),
        ));
    }
    let pattern = &match_clause.patterns[0];

    // Build the variable bindings produced by the pattern, then apply the
    // trailing WHERE (if any), then project the RETURN items.
    let bindings = match pattern.steps.len() {
        0 => match_single_node(store, &pattern.start)?,
        1 => match_one_hop(store, &pattern.start, &pattern.steps[0])?,
        _ => {
            return Err(ExecError::UnsupportedShape(
                "only single-node and one-hop patterns are supported".into(),
            ));
        }
    };

    let filtered = apply_where(bindings, match_clause.where_clause.as_ref())?;
    project(filtered, projection)
}

/// A set of variable→binding maps, one per matched pattern instance.
type Scopes = Vec<BTreeMap<String, Binding>>;

fn split_match_return(query: &Query) -> Result<(&MatchClause, &ProjectionClause), ExecError> {
    if query.clauses.len() != 2 {
        return Err(ExecError::UnsupportedShape(
            "expected exactly `MATCH ... RETURN ...`".into(),
        ));
    }
    let match_clause = match &query.clauses[0] {
        Clause::Match(m) if !m.optional => m,
        Clause::Match(_) => {
            return Err(ExecError::UnsupportedShape(
                "OPTIONAL MATCH is not supported by the demo".into(),
            ));
        }
        _ => {
            return Err(ExecError::UnsupportedShape(
                "first clause must be MATCH".into(),
            ));
        }
    };
    let projection = match &query.clauses[1] {
        Clause::Return(p) => p,
        _ => {
            return Err(ExecError::UnsupportedShape(
                "second clause must be RETURN".into(),
            ));
        }
    };
    Ok((match_clause, projection))
}

/// `MATCH (n:Label {k: v}) RETURN ...` — scan nodes, filtered by label and the
/// inline property map.
fn match_single_node(store: &GraphStore, pattern: &NodePattern) -> Result<Scopes, ExecError> {
    let var = pattern.variable.clone();
    let mut scopes = Scopes::new();
    for node in store.nodes() {
        if node_matches(node, pattern)? {
            let mut scope = BTreeMap::new();
            if let Some(v) = &var {
                scope.insert(v.clone(), Binding::Node(node.clone()));
            }
            scopes.push(scope);
        }
    }
    Ok(scopes)
}

/// `MATCH (a)-[:REL]->(b) RETURN ...` — for each edge whose endpoints match the
/// node patterns and whose type matches the rel pattern, bind `a`, `b`, and the
/// relationship variable (if named). Undirected (`--`) and incoming (`<--`) are
/// handled by orienting the endpoint patterns to the edge direction.
fn match_one_hop(
    store: &GraphStore,
    start: &NodePattern,
    step: &crate::cypher::ast::PatternStep,
) -> Result<Scopes, ExecError> {
    let rel = &step.relationship;
    let end = &step.node;

    let mut scopes = Scopes::new();
    for edge in store.edges() {
        if !rel_type_matches(edge, rel) {
            continue;
        }

        // Determine which endpoint plays `start` and which plays `end` given the
        // relationship direction. For Outgoing: start=source, end=target. For
        // Incoming: start=target, end=source. Undirected tries both.
        let orientations: &[(bool, bool)] = match rel.direction {
            Direction::Outgoing => &[(true, false)],
            Direction::Incoming => &[(false, true)],
            Direction::Undirected => &[(true, false), (false, true)],
        };

        for &(start_is_source, _) in orientations {
            let (start_node_id, end_node_id) = if start_is_source {
                (edge.source, edge.target)
            } else {
                (edge.target, edge.source)
            };
            let (Some(start_node), Some(end_node)) =
                (store.node(start_node_id), store.node(end_node_id))
            else {
                continue;
            };
            if node_matches(start_node, start)? && node_matches(end_node, end)? {
                let mut scope = BTreeMap::new();
                if let Some(v) = &start.variable {
                    scope.insert(v.clone(), Binding::Node(start_node.clone()));
                }
                if let Some(v) = &end.variable {
                    scope.insert(v.clone(), Binding::Node(end_node.clone()));
                }
                if let Some(v) = &rel.variable {
                    scope.insert(v.clone(), Binding::Edge(edge.clone()));
                }
                scopes.push(scope);
            }
        }
    }
    Ok(scopes)
}

/// Does `node` satisfy the pattern's label filter and inline property equalities?
fn node_matches(node: &Node, pattern: &NodePattern) -> Result<bool, ExecError> {
    for label in &pattern.labels {
        if !node.has_label(label) {
            return Ok(false);
        }
    }
    if let Some(props) = &pattern.properties {
        for (key, expr) in props {
            let expected = literal_value(expr)?;
            match node.property(key) {
                Some(actual) if actual.cypher_equal(&expected) == Some(true) => {}
                _ => return Ok(false),
            }
        }
    }
    Ok(true)
}

/// Does the edge's type satisfy the relationship pattern's type filter? An empty
/// type list matches any relationship.
fn rel_type_matches(edge: &Edge, rel: &crate::cypher::ast::RelPattern) -> bool {
    rel.types.is_empty() || rel.types.iter().any(|t| edge.has_type(t))
}

/// Apply a trailing `WHERE var.key = <literal>` equality filter, if present.
fn apply_where(scopes: Scopes, where_clause: Option<&Expr>) -> Result<Scopes, ExecError> {
    let Some(expr) = where_clause else {
        return Ok(scopes);
    };
    let mut kept = Scopes::new();
    for scope in scopes {
        if eval_predicate(expr, &scope)? {
            kept.push(scope);
        }
    }
    Ok(kept)
}

/// Evaluate a `WHERE` predicate against one scope. The demo supports a single
/// equality `var.key = <literal>` (and its commuted form).
fn eval_predicate(expr: &Expr, scope: &BTreeMap<String, Binding>) -> Result<bool, ExecError> {
    match expr {
        Expr::Binary {
            op: BinaryOp::Equal,
            lhs,
            rhs,
        } => {
            let l = eval_scalar(lhs, scope)?;
            let r = eval_scalar(rhs, scope)?;
            // openCypher `=` is ternary; WHERE keeps a row only on Some(true).
            Ok(l.cypher_equal(&r) == Some(true))
        }
        _ => Err(ExecError::UnsupportedExpression(
            "WHERE supports only a single `var.key = <literal>` equality".into(),
        )),
    }
}

/// Evaluate a scalar expression — either a literal or a `var.key` property
/// access against the current scope — to a [`PropertyValue`].
fn eval_scalar(expr: &Expr, scope: &BTreeMap<String, Binding>) -> Result<PropertyValue, ExecError> {
    match expr {
        Expr::Literal(v) => Ok(v.clone()),
        Expr::Property { base, key } => {
            let Expr::Variable(var) = base.as_ref() else {
                return Err(ExecError::UnsupportedExpression(
                    "property access must be on a bound variable".into(),
                ));
            };
            let binding = scope
                .get(var)
                .ok_or_else(|| ExecError::UnknownVariable(var.clone()))?;
            let value = match binding {
                Binding::Node(n) => n.property(key),
                Binding::Edge(e) => e.property(key),
            };
            Ok(value.cloned().unwrap_or(PropertyValue::Null))
        }
        _ => Err(ExecError::UnsupportedExpression(
            "only literals and `var.key` property access are supported".into(),
        )),
    }
}

/// Reduce an inline-property-map value expression to a constant. Inline filters
/// in the demo are literal-valued (`{name: 'Alice'}`).
fn literal_value(expr: &Expr) -> Result<PropertyValue, ExecError> {
    match expr {
        Expr::Literal(v) => Ok(v.clone()),
        _ => Err(ExecError::UnsupportedExpression(
            "inline property filters must be literal values".into(),
        )),
    }
}

/// Project the `RETURN` items out of each scope into result rows.
fn project(scopes: Scopes, projection: &ProjectionClause) -> Result<Vec<Row>, ExecError> {
    let items = match &projection.body {
        ReturnBody::Items(items) => items,
        ReturnBody::All { .. } => {
            return Err(ExecError::UnsupportedShape(
                "RETURN * is not supported by the demo; name your variables".into(),
            ));
        }
    };

    let mut rows = Vec::with_capacity(scopes.len());
    for scope in &scopes {
        let mut row: Row = Vec::with_capacity(items.len());
        for item in items {
            let Expr::Variable(var) = &item.expr else {
                return Err(ExecError::UnsupportedExpression(
                    "RETURN supports only bare variable references in the demo".into(),
                ));
            };
            let binding = scope
                .get(var)
                .ok_or_else(|| ExecError::UnknownVariable(var.clone()))?;
            let column = item.alias.clone().unwrap_or_else(|| var.clone());
            row.push((column, binding.clone()));
        }
        rows.push(row);
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cypher::parse;

    /// Build the demo graph: Alice & Bob (Person), Alice-[:KNOWS]->Bob.
    fn demo_graph() -> GraphStore {
        let mut g = GraphStore::new();
        let alice = g.insert_node(["Person"], [("name", "Alice"), ("city", "Berlin")]);
        let bob = g.insert_node(["Person"], [("name", "Bob")]);
        // Add an age to Alice as an integer to exercise mixed types.
        let alice2 = g.insert_node(["Robot"], [("name", "C3PO")]);
        let _ = alice2;
        g.insert_edge("KNOWS", alice, bob, Vec::<(String, PropertyValue)>::new());
        g
    }

    fn node_name(b: &Binding) -> Option<String> {
        match b {
            Binding::Node(n) => match n.property("name") {
                Some(PropertyValue::String(s)) => Some(s.clone()),
                _ => None,
            },
            Binding::Edge(_) => None,
        }
    }

    #[test]
    fn single_node_label_and_property_filter() {
        let g = demo_graph();
        let q = parse("MATCH (p:Person {name: 'Alice'}) RETURN p").expect("parse");
        let rows = execute(&g, &q).expect("execute");
        assert_eq!(rows.len(), 1, "exactly Alice matches");
        let (col, binding) = &rows[0][0];
        assert_eq!(col, "p");
        assert_eq!(node_name(binding).as_deref(), Some("Alice"));
    }

    #[test]
    fn single_node_label_only_matches_all_of_label() {
        let g = demo_graph();
        let q = parse("MATCH (p:Person) RETURN p").expect("parse");
        let rows = execute(&g, &q).expect("execute");
        assert_eq!(rows.len(), 2, "Alice and Bob are Persons; the Robot is not");
    }

    #[test]
    fn single_node_property_filter_with_no_match_is_empty() {
        let g = demo_graph();
        let q = parse("MATCH (p:Person {name: 'Nobody'}) RETURN p").expect("parse");
        let rows = execute(&g, &q).expect("execute");
        assert!(rows.is_empty());
    }

    #[test]
    fn one_hop_returns_endpoint_pair() {
        let g = demo_graph();
        let q = parse("MATCH (a:Person)-[:KNOWS]->(b) RETURN a, b").expect("parse");
        let rows = execute(&g, &q).expect("execute");
        assert_eq!(rows.len(), 1, "the single KNOWS edge");
        let row = &rows[0];
        assert_eq!(row[0].0, "a");
        assert_eq!(row[1].0, "b");
        assert_eq!(node_name(&row[0].1).as_deref(), Some("Alice"));
        assert_eq!(node_name(&row[1].1).as_deref(), Some("Bob"));
    }

    #[test]
    fn one_hop_wrong_rel_type_matches_nothing() {
        let g = demo_graph();
        let q = parse("MATCH (a:Person)-[:LIKES]->(b) RETURN a, b").expect("parse");
        let rows = execute(&g, &q).expect("execute");
        assert!(rows.is_empty());
    }

    #[test]
    fn where_equality_filter() {
        let g = demo_graph();
        let q = parse("MATCH (p:Person) WHERE p.name = 'Bob' RETURN p").expect("parse");
        let rows = execute(&g, &q).expect("execute");
        assert_eq!(rows.len(), 1);
        assert_eq!(node_name(&rows[0][0].1).as_deref(), Some("Bob"));
    }

    #[test]
    fn return_alias_renames_column() {
        let g = demo_graph();
        let q = parse("MATCH (p:Person {name: 'Alice'}) RETURN p AS person").expect("parse");
        let rows = execute(&g, &q).expect("execute");
        assert_eq!(rows[0][0].0, "person");
    }

    #[test]
    fn unsupported_shape_is_reported_not_wrong() {
        let g = demo_graph();
        // Two-hop is outside the demo surface.
        let q = parse("MATCH (a)-[:KNOWS]->(b)-[:KNOWS]->(c) RETURN a, c").expect("parse");
        assert!(matches!(
            execute(&g, &q),
            Err(ExecError::UnsupportedShape(_))
        ));
    }

    #[test]
    fn return_unknown_variable_errors() {
        let g = demo_graph();
        let q = parse("MATCH (p:Person) RETURN q").expect("parse");
        assert!(matches!(
            execute(&g, &q),
            Err(ExecError::UnknownVariable(_))
        ));
    }
}
