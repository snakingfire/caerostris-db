//! The end-to-end demo: an in-memory graph store, a tiny `MATCH ... RETURN`
//! executor over it, and a scripted driver that proves the round trip
//! **insert → query → see the inserted data returned**.
//!
//! This is the hackathon-video deliverable. It is intentionally minimal and
//! sits *beside* the real engine layers (durable storage, the full planner,
//! transactions) rather than replacing them: it wires the already-landed
//! [`crate::model`] and [`crate::cypher`] pieces into something runnable today.
//!
//! - [`store::GraphStore`] — `Vec<Node>` + `Vec<Edge>`, with `insert_node` /
//!   `insert_edge`.
//! - [`executor::execute`] — runs a parsed [`crate::cypher::ast::Query`] for the
//!   single-node and one-hop `MATCH` shapes and returns bound rows.
//! - [`run_demo`] — builds the canonical demo graph, runs two queries, and
//!   writes labelled, human-readable output for screen recording.

pub mod executor;
pub mod store;

use std::fmt::Write as _;

use crate::cypher::parse;
use crate::model::PropertyValue;

pub use executor::{Binding, ExecError, Row, execute};
pub use store::GraphStore;

/// Render a [`PropertyValue`] the way the demo prints it (close to Cypher
/// literal syntax: strings quoted, `null` lower-case).
fn render_value(value: &PropertyValue) -> String {
    match value {
        PropertyValue::Null => "null".to_string(),
        PropertyValue::Boolean(b) => b.to_string(),
        PropertyValue::Integer(i) => i.to_string(),
        PropertyValue::Float(f) => f.to_string(),
        PropertyValue::String(s) => format!("'{s}'"),
        PropertyValue::List(items) => {
            let inner: Vec<String> = items.iter().map(render_value).collect();
            format!("[{}]", inner.join(", "))
        }
        PropertyValue::Map(map) => {
            let inner: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{k}: {}", render_value(v)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
    }
}

/// Render a binding as a human-readable Cypher-ish string, e.g.
/// `(:Person {name: 'Alice', city: 'Berlin'})` for a node, or
/// `[:KNOWS {since: 2020}]` for an edge.
#[must_use]
pub fn render_binding(binding: &Binding) -> String {
    match binding {
        Binding::Node(node) => {
            let mut out = String::from("(");
            for label in &node.labels {
                let _ = write!(out, ":{label}");
            }
            if !node.properties.is_empty() {
                if !node.labels.is_empty() {
                    out.push(' ');
                }
                let props: Vec<String> = node
                    .properties
                    .iter()
                    .map(|(k, v)| format!("{k}: {}", render_value(v)))
                    .collect();
                let _ = write!(out, "{{{}}}", props.join(", "));
            }
            out.push(')');
            out
        }
        Binding::Edge(edge) => {
            let mut out = format!("[:{}", edge.rel_type);
            if !edge.properties.is_empty() {
                let props: Vec<String> = edge
                    .properties
                    .iter()
                    .map(|(k, v)| format!("{k}: {}", render_value(v)))
                    .collect();
                let _ = write!(out, " {{{}}}", props.join(", "));
            }
            out.push(']');
            out
        }
    }
}

/// Render a full result set as labelled rows: one line per row, columns shown as
/// `name = <binding>`.
#[must_use]
pub fn render_rows(rows: &[Row]) -> String {
    if rows.is_empty() {
        return "  (no rows)".to_string();
    }
    let mut out = String::new();
    for (i, row) in rows.iter().enumerate() {
        let cols: Vec<String> = row
            .iter()
            .map(|(name, binding)| format!("{name} = {}", render_binding(binding)))
            .collect();
        let _ = writeln!(out, "  row {}: {}", i + 1, cols.join(", "));
    }
    // Trim the trailing newline so callers control spacing.
    out.pop();
    out
}

/// Build the canonical demo graph and run the two demo queries, writing
/// labelled, human-readable output to `out`.
///
/// The graph: `(:Person {name: 'Alice', age: 30})`,
/// `(:Person {name: 'Bob'})`, and `Alice-[:KNOWS]->Bob`.
///
/// # Errors
/// Returns an [`ExecError`] only if the hard-coded demo queries fail to execute
/// — which would indicate a regression in the parser or executor, so the demo
/// surfaces it rather than printing a false success.
///
/// # Panics
/// Never panics on the hard-coded queries; the `expect` on `parse` would only
/// fire if the bundled, constant query strings stopped parsing, which the unit
/// tests guard against.
pub fn run_demo(out: &mut impl std::io::Write) -> Result<(), Box<dyn std::error::Error>> {
    let mut graph = GraphStore::new();

    writeln!(out, "== caerostris-db end-to-end demo ==")?;
    writeln!(out)?;
    writeln!(out, "-- 1. Insert data --")?;
    let alice = graph.insert_node(
        ["Person"],
        vec![
            ("name", PropertyValue::String("Alice".into())),
            ("age", PropertyValue::Integer(30)),
        ],
    );
    writeln!(
        out,
        "  inserted (:Person {{name: 'Alice', age: 30}}) -> id {}",
        alice.get()
    )?;
    let bob = graph.insert_node(
        ["Person"],
        vec![("name", PropertyValue::String("Bob".into()))],
    );
    writeln!(
        out,
        "  inserted (:Person {{name: 'Bob'}}) -> id {}",
        bob.get()
    )?;
    let edge = graph.insert_edge("KNOWS", alice, bob, Vec::<(String, PropertyValue)>::new());
    writeln!(
        out,
        "  inserted (Alice)-[:KNOWS]->(Bob) -> edge id {}",
        edge.get()
    )?;
    writeln!(out)?;

    // Query 1: single-node label + property filter.
    let q1 = "MATCH (p:Person {name: 'Alice'}) RETURN p";
    writeln!(out, "-- 2. Query a single node --")?;
    writeln!(out, "  query: {q1}")?;
    let query1 = parse(q1).expect("bundled demo query 1 parses");
    let rows1 = execute(&graph, &query1)?;
    writeln!(out, "  result:")?;
    writeln!(out, "{}", render_rows(&rows1))?;
    writeln!(out)?;

    // Query 2: one-hop traversal returning both endpoints.
    let q2 = "MATCH (a:Person)-[:KNOWS]->(b) RETURN a, b";
    writeln!(out, "-- 3. Query a one-hop relationship --")?;
    writeln!(out, "  query: {q2}")?;
    let query2 = parse(q2).expect("bundled demo query 2 parses");
    let rows2 = execute(&graph, &query2)?;
    writeln!(out, "  result:")?;
    writeln!(out, "{}", render_rows(&rows2))?;
    writeln!(out)?;

    writeln!(out, "== demo complete: inserted data returned by MATCH ==")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, Node, NodeId};

    #[test]
    fn render_node_binding_is_cypher_ish() {
        let node = Node::new(NodeId(0))
            .with_label("Person")
            .with_property("name", "Alice");
        let rendered = render_binding(&Binding::Node(node));
        assert_eq!(rendered, "(:Person {name: 'Alice'})");
    }

    #[test]
    fn render_edge_binding_shows_type() {
        let edge = Edge::new(0_u64, "KNOWS", 0_u64, 1_u64).with_property("since", 2020_i64);
        let rendered = render_binding(&Binding::Edge(edge));
        assert_eq!(rendered, "[:KNOWS {since: 2020}]");
    }

    #[test]
    fn render_rows_empty_says_no_rows() {
        assert_eq!(render_rows(&[]), "  (no rows)");
    }

    #[test]
    fn run_demo_emits_inserted_data_and_query_results() {
        let mut buf: Vec<u8> = Vec::new();
        run_demo(&mut buf).expect("demo runs end to end");
        let text = String::from_utf8(buf).expect("utf8 output");

        // Insert phase shows the inserted data.
        assert!(text.contains("inserted (:Person {name: 'Alice', age: 30})"));
        assert!(text.contains("(Alice)-[:KNOWS]->(Bob)"));

        // Query 1 returns Alice.
        assert!(text.contains("MATCH (p:Person {name: 'Alice'}) RETURN p"));
        assert!(text.contains("p = (:Person {age: 30, name: 'Alice'})"));

        // Query 2 returns the Alice/Bob pair.
        assert!(text.contains("MATCH (a:Person)-[:KNOWS]->(b) RETURN a, b"));
        assert!(text.contains("a = (:Person {age: 30, name: 'Alice'})"));
        assert!(text.contains("b = (:Person {name: 'Bob'})"));

        assert!(text.contains("demo complete"));
    }
}
