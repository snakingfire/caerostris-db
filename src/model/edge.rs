//! Graph edges (relationships): the directed, typed connections between nodes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{NodeId, PropertyValue};

/// A stable, engine-assigned relationship identifier.
///
/// Distinct newtype from [`NodeId`](super::NodeId) so an edge id and a node id
/// can never be confused at a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EdgeId(pub u64);

impl EdgeId {
    /// The raw `u64` behind this id.
    #[must_use]
    pub fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for EdgeId {
    fn from(v: u64) -> Self {
        EdgeId(v)
    }
}

/// An edge (relationship): a directed, typed connection from a source node to a
/// target node, carrying its own properties.
///
/// In openCypher a relationship has **exactly one** type (unlike a node, which
/// has a *set* of labels) and is always **directed** — it has a distinct source
/// (`(src)-[r]->(dst)`) and target. Undirected matching is a query-time concern;
/// the stored edge is directed. Properties are a string-keyed [`BTreeMap`] of
/// [`PropertyValue`]s, deterministic in iteration/serialisation order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    /// The relationship's stable identity.
    pub id: EdgeId,
    /// The relationship type (e.g. `"KNOWS"`). Exactly one per edge.
    pub rel_type: String,
    /// The source (tail) node — the `(src)` in `(src)-[r]->(dst)`.
    pub source: NodeId,
    /// The target (head) node — the `(dst)` in `(src)-[r]->(dst)`.
    pub target: NodeId,
    /// The relationship's properties.
    pub properties: BTreeMap<String, PropertyValue>,
}

impl Edge {
    /// An edge of the given id and type, directed `source -> target`, with no
    /// properties.
    #[must_use]
    pub fn new(
        id: impl Into<EdgeId>,
        rel_type: impl Into<String>,
        source: impl Into<NodeId>,
        target: impl Into<NodeId>,
    ) -> Self {
        Edge {
            id: id.into(),
            rel_type: rel_type.into(),
            source: source.into(),
            target: target.into(),
            properties: BTreeMap::new(),
        }
    }

    /// Builder: set a property, returning `self` for chaining. Overwrites an
    /// existing key.
    #[must_use]
    pub fn with_property(
        mut self,
        key: impl Into<String>,
        value: impl Into<PropertyValue>,
    ) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }

    /// `true` if this edge's type equals `rel_type`.
    #[must_use]
    pub fn has_type(&self, rel_type: &str) -> bool {
        self.rel_type == rel_type
    }

    /// The value of property `key`, or `None` if absent. See
    /// [`Node::property`](super::Node::property) for the missing-vs-null
    /// distinction.
    #[must_use]
    pub fn property(&self, key: &str) -> Option<&PropertyValue> {
        self.properties.get(key)
    }
}

// ---- Unit tests -------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_edge_has_correct_fields() {
        let e = Edge::new(1_u64, "KNOWS", 10_u64, 20_u64);
        assert_eq!(e.id, EdgeId(1));
        assert_eq!(e.rel_type, "KNOWS");
        assert_eq!(e.source, NodeId(10));
        assert_eq!(e.target, NodeId(20));
        assert!(e.properties.is_empty());
    }

    #[test]
    fn edge_type_check() {
        let e = Edge::new(1_u64, "LIKES", 1_u64, 2_u64);
        assert!(e.has_type("LIKES"));
        assert!(!e.has_type("KNOWS"));
    }

    #[test]
    fn edge_with_property_and_lookup() {
        let e = Edge::new(2_u64, "RATED", 1_u64, 2_u64).with_property("score", 5_i64);
        assert_eq!(e.property("score"), Some(&PropertyValue::Integer(5)));
        assert_eq!(e.property("absent"), None);
    }

    #[test]
    fn edge_id_newtype_get() {
        assert_eq!(EdgeId(99).get(), 99_u64);
        assert_eq!(EdgeId::from(3_u64), EdgeId(3));
    }

    #[test]
    fn serde_roundtrip_edge() {
        let e = Edge::new(7_u64, "FOLLOWS", 100_u64, 200_u64)
            .with_property("since", "2024".to_string());
        let json = serde_json::to_string(&e).expect("serialize");
        let back: Edge = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(e, back);
    }
}
