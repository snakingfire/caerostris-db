//! A minimal, in-memory property-graph store for the end-to-end demo.
//!
//! This is deliberately the simplest thing that demonstrates the engine's
//! value proposition end-to-end: insert nodes and edges, then query them back
//! with openCypher `MATCH`. It is **not** the durable, object-storage-native
//! store (that lands via SPIKE-0003 / the storage cascade); it holds the graph
//! in two `Vec`s in memory. The demo binary and `scripts/demo.sh` drive it.
//!
//! Ids are assigned monotonically from zero, separately for nodes and edges, so
//! a `NodeId` and an `EdgeId` with the same raw value are still distinct types
//! (see [`crate::model::NodeId`] / [`crate::model::EdgeId`]).

use crate::model::{Edge, EdgeId, Node, NodeId, PropertyValue};

/// An in-memory property graph: a flat list of nodes and a flat list of edges.
///
/// Lookups are linear scans — fine for the demo's handful of nodes, and the
/// shape the [`crate::demo::executor`] expects. The durable engine replaces
/// this with the object-storage format and secondary indexes.
#[derive(Debug, Default, Clone)]
pub struct GraphStore {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    next_node: u64,
    next_edge: u64,
}

impl GraphStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a node with the given labels and properties, returning its
    /// freshly assigned [`NodeId`].
    ///
    /// Labels and properties are taken as iterators so callers can pass arrays,
    /// `Vec`s, or any other iterable without allocating an intermediate
    /// collection first.
    pub fn insert_node(
        &mut self,
        labels: impl IntoIterator<Item = impl Into<String>>,
        properties: impl IntoIterator<Item = (impl Into<String>, impl Into<PropertyValue>)>,
    ) -> NodeId {
        let id = NodeId(self.next_node);
        self.next_node += 1;
        let mut node = Node::new(id);
        for label in labels {
            node = node.with_label(label);
        }
        for (key, value) in properties {
            node = node.with_property(key, value);
        }
        self.nodes.push(node);
        id
    }

    /// Insert a directed, typed edge `source -[type]-> target`, returning its
    /// freshly assigned [`EdgeId`].
    pub fn insert_edge(
        &mut self,
        rel_type: impl Into<String>,
        source: NodeId,
        target: NodeId,
        properties: impl IntoIterator<Item = (impl Into<String>, impl Into<PropertyValue>)>,
    ) -> EdgeId {
        let id = EdgeId(self.next_edge);
        self.next_edge += 1;
        let mut edge = Edge::new(id, rel_type, source, target);
        for (key, value) in properties {
            edge = edge.with_property(key, value);
        }
        self.edges.push(edge);
        id
    }

    /// All stored nodes, in insertion order.
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// All stored edges, in insertion order.
    #[must_use]
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Look up a node by id (linear scan).
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_node_assigns_sequential_ids_and_stores_fields() {
        let mut g = GraphStore::new();
        let alice = g.insert_node(["Person"], [("name", "Alice"), ("city", "Berlin")]);
        let bob = g.insert_node(["Person"], [("name", "Bob")]);

        assert_eq!(alice, NodeId(0));
        assert_eq!(bob, NodeId(1));
        assert_eq!(g.nodes().len(), 2);

        let a = g.node(alice).expect("alice exists");
        assert!(a.has_label("Person"));
        assert_eq!(
            a.property("name"),
            Some(&PropertyValue::String("Alice".into()))
        );
        assert_eq!(
            a.property("city"),
            Some(&PropertyValue::String("Berlin".into()))
        );
    }

    #[test]
    fn insert_node_supports_mixed_property_value_types() {
        let mut g = GraphStore::new();
        // The property-value `Into` covers &str and i64; exercise both.
        let id = g.insert_node(
            ["Person"],
            vec![
                ("name", PropertyValue::String("Alice".into())),
                ("age", PropertyValue::Integer(30)),
            ],
        );
        let n = g.node(id).expect("node exists");
        assert_eq!(n.property("age"), Some(&PropertyValue::Integer(30)));
    }

    #[test]
    fn insert_edge_links_two_nodes_with_type() {
        let mut g = GraphStore::new();
        let alice = g.insert_node(["Person"], [("name", "Alice")]);
        let bob = g.insert_node(["Person"], [("name", "Bob")]);
        let e = g.insert_edge("KNOWS", alice, bob, [("since", 2020_i64)]);

        assert_eq!(e, EdgeId(0));
        assert_eq!(g.edges().len(), 1);
        let edge = &g.edges()[0];
        assert!(edge.has_type("KNOWS"));
        assert_eq!(edge.source, alice);
        assert_eq!(edge.target, bob);
        assert_eq!(edge.property("since"), Some(&PropertyValue::Integer(2020)));
    }

    #[test]
    fn node_and_edge_ids_advance_independently() {
        let mut g = GraphStore::new();
        let a = g.insert_node(["X"], Vec::<(String, PropertyValue)>::new());
        let b = g.insert_node(["X"], Vec::<(String, PropertyValue)>::new());
        let e = g.insert_edge("R", a, b, Vec::<(String, PropertyValue)>::new());
        // Edge ids start at 0 too — distinct counter from node ids.
        assert_eq!(e, EdgeId(0));
        assert_eq!(b, NodeId(1));
    }
}
