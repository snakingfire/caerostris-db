//! Persisting the demo graph to (and loading it back from) an
//! [`ObjectStore`](crate::storage::ObjectStore) — the bridge that turns the
//! in-memory [`GraphStore`] into durable object-storage state.
//!
//! This is the demo keystone: it proves the *object-storage-native* claim by
//! writing each node and edge as its own object and then answering openCypher
//! `MATCH` queries by reading those objects back. With an
//! [`S3CliStore`](crate::storage::S3CliStore) the objects are real keys in a
//! real S3/MinIO bucket; with a [`MemoryStore`](crate::storage::MemoryStore)
//! the exact same code path runs in unit tests.
//!
//! ## Layout
//!
//! Each entity is one JSON object, keyed by id so it is independently
//! addressable (the shape a columnar store later replaces, but legible for the
//! demo):
//!
//! ```text
//! nodes/<id>.json   -> serde_json of crate::model::Node
//! edges/<id>.json   -> serde_json of crate::model::Edge
//! ```
//!
//! Loading lists `nodes/` and `edges/`, fetches each object, deserialises it,
//! and rebuilds a [`GraphStore`] preserving the original ids.

use crate::model::{Edge, Node};
use crate::storage::{ObjectStore, StoreError};

use super::store::GraphStore;

/// Object-key prefix under which node objects are stored.
pub const NODES_PREFIX: &str = "nodes/";
/// Object-key prefix under which edge objects are stored.
pub const EDGES_PREFIX: &str = "edges/";

/// Errors raised while persisting or loading a graph.
#[derive(Debug)]
pub enum PersistError {
    /// The underlying object store failed.
    Store(StoreError),
    /// A stored object could not be (de)serialised as JSON.
    Serde(serde_json::Error),
}

impl std::fmt::Display for PersistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PersistError::Store(e) => write!(f, "object store error: {e}"),
            PersistError::Serde(e) => write!(f, "serialisation error: {e}"),
        }
    }
}

impl std::error::Error for PersistError {}

impl From<StoreError> for PersistError {
    fn from(e: StoreError) -> Self {
        PersistError::Store(e)
    }
}

impl From<serde_json::Error> for PersistError {
    fn from(e: serde_json::Error) -> Self {
        PersistError::Serde(e)
    }
}

/// The object key a node is stored under (`nodes/<id>.json`).
#[must_use]
pub fn node_key(node: &Node) -> String {
    format!("{NODES_PREFIX}{}.json", node.id.get())
}

/// The object key an edge is stored under (`edges/<id>.json`).
#[must_use]
pub fn edge_key(edge: &Edge) -> String {
    format!("{EDGES_PREFIX}{}.json", edge.id.get())
}

/// Persist every node and edge of `graph` into `store` as individual JSON
/// objects, returning the keys written (nodes then edges, in store order).
///
/// # Errors
/// Returns [`PersistError`] if serialisation or a store `put` fails.
pub fn persist_graph(
    store: &mut dyn ObjectStore,
    graph: &GraphStore,
) -> Result<Vec<String>, PersistError> {
    let mut written = Vec::with_capacity(graph.nodes().len() + graph.edges().len());
    for node in graph.nodes() {
        let key = node_key(node);
        let bytes = serde_json::to_vec_pretty(node)?;
        store.put(&key, bytes)?;
        written.push(key);
    }
    for edge in graph.edges() {
        let key = edge_key(edge);
        let bytes = serde_json::to_vec_pretty(edge)?;
        store.put(&key, bytes)?;
        written.push(key);
    }
    Ok(written)
}

/// Reconstruct a [`GraphStore`] by reading every `nodes/` and `edges/` object
/// back out of `store`.
///
/// The rebuilt graph preserves the original node/edge ids (so queries return
/// the same entities that were persisted).
///
/// # Errors
/// Returns [`PersistError`] if a `list`/`get` fails or an object is not valid
/// JSON for its type.
pub fn load_graph(store: &dyn ObjectStore) -> Result<GraphStore, PersistError> {
    let mut nodes = Vec::new();
    for key in store.list(NODES_PREFIX)? {
        let bytes = store.get(&key)?;
        let node: Node = serde_json::from_slice(&bytes)?;
        nodes.push(node);
    }
    let mut edges = Vec::new();
    for key in store.list(EDGES_PREFIX)? {
        let bytes = store.get(&key)?;
        let edge: Edge = serde_json::from_slice(&bytes)?;
        edges.push(edge);
    }
    Ok(GraphStore::from_parts(nodes, edges))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PropertyValue;
    use crate::storage::MemoryStore;

    fn sample_graph() -> GraphStore {
        let mut g = GraphStore::new();
        let alice = g.insert_node(
            ["Person"],
            vec![
                ("name", PropertyValue::String("Alice".into())),
                ("age", PropertyValue::Integer(30)),
            ],
        );
        let bob = g.insert_node(["Person"], [("name", "Bob")]);
        g.insert_edge("KNOWS", alice, bob, [("since", 2020_i64)]);
        g
    }

    #[test]
    fn keys_are_id_addressed() {
        let g = sample_graph();
        assert_eq!(node_key(&g.nodes()[0]), "nodes/0.json");
        assert_eq!(node_key(&g.nodes()[1]), "nodes/1.json");
        assert_eq!(edge_key(&g.edges()[0]), "edges/0.json");
    }

    #[test]
    fn persist_writes_one_object_per_entity() {
        let g = sample_graph();
        let mut store = MemoryStore::new();
        let keys = persist_graph(&mut store, &g).expect("persist");
        assert_eq!(keys, vec!["nodes/0.json", "nodes/1.json", "edges/0.json"]);
        assert_eq!(store.len(), 3);
        // The node objects really hold the data (legible JSON).
        let alice_bytes = store.get("nodes/0.json").expect("alice object");
        let text = String::from_utf8(alice_bytes).expect("utf8");
        assert!(text.contains("Alice"));
    }

    #[test]
    fn round_trip_preserves_ids_labels_props_and_edges() {
        let g = sample_graph();
        let mut store = MemoryStore::new();
        persist_graph(&mut store, &g).expect("persist");

        let loaded = load_graph(&store).expect("load");
        assert_eq!(loaded.nodes().len(), 2);
        assert_eq!(loaded.edges().len(), 1);

        // Ids preserved.
        assert_eq!(loaded.nodes()[0].id, g.nodes()[0].id);
        // Properties preserved.
        let alice = &loaded.nodes()[0];
        assert!(alice.has_label("Person"));
        assert_eq!(alice.property("age"), Some(&PropertyValue::Integer(30)));
        // Edge endpoints preserved.
        let edge = &loaded.edges()[0];
        assert!(edge.has_type("KNOWS"));
        assert_eq!(edge.source, g.nodes()[0].id);
        assert_eq!(edge.target, g.nodes()[1].id);
        assert_eq!(edge.property("since"), Some(&PropertyValue::Integer(2020)));
    }

    #[test]
    fn load_from_empty_store_is_empty_graph() {
        let store = MemoryStore::new();
        let loaded = load_graph(&store).expect("load empty");
        assert!(loaded.nodes().is_empty());
        assert!(loaded.edges().is_empty());
    }
}
