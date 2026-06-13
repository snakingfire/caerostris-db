//! Graph nodes: the vertices of the property graph.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::PropertyValue;

/// A stable, engine-assigned node identifier.
///
/// A `NodeId` is opaque: it is *not* a property and carries no openCypher
/// meaning beyond identity. Storage assigns it; the value type system
/// ([`PropertyValue`]) never holds one. Wrapping `u64` (rather than using a bare
/// integer) keeps node ids and edge ids from being silently interchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u64);

impl NodeId {
    /// The raw `u64` behind this id.
    #[must_use]
    pub fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for NodeId {
    fn from(v: u64) -> Self {
        NodeId(v)
    }
}

/// A node (vertex): an identity, a set of labels, and a property map.
///
/// In openCypher a node carries **zero or more labels** (an unordered set —
/// `:Person:Employee` is the same node regardless of label order) and a map of
/// string-keyed [`PropertyValue`]s. Labels are stored in a [`BTreeSet`] so they
/// are deduplicated and serialise deterministically; properties in a
/// [`BTreeMap`] for the same reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// The node's stable identity.
    pub id: NodeId,
    /// The node's labels (an unordered, deduplicated set).
    pub labels: BTreeSet<String>,
    /// The node's properties.
    pub properties: BTreeMap<String, PropertyValue>,
}

impl Node {
    /// A node with the given id, no labels, and no properties.
    #[must_use]
    pub fn new(id: impl Into<NodeId>) -> Self {
        Node {
            id: id.into(),
            labels: BTreeSet::new(),
            properties: BTreeMap::new(),
        }
    }

    /// Builder: add a label, returning `self` for chaining. Adding a label that
    /// is already present is a no-op (labels are a set).
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.labels.insert(label.into());
        self
    }

    /// Builder: set a property, returning `self` for chaining. Setting a key
    /// that already exists overwrites it.
    #[must_use]
    pub fn with_property(
        mut self,
        key: impl Into<String>,
        value: impl Into<PropertyValue>,
    ) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }

    /// `true` if the node carries `label`.
    #[must_use]
    pub fn has_label(&self, label: &str) -> bool {
        self.labels.contains(label)
    }

    /// The value of property `key`, or `None` if the node has no such property.
    ///
    /// Note the openCypher distinction: a *missing* property returns `None`
    /// here, whereas a property explicitly set to [`PropertyValue::Null`]
    /// returns `Some(&PropertyValue::Null)`.
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
    fn new_node_has_no_labels_or_properties() {
        let n = Node::new(1_u64);
        assert_eq!(n.id, NodeId(1));
        assert!(n.labels.is_empty());
        assert!(n.properties.is_empty());
    }

    #[test]
    fn builder_adds_labels_deduplicates() {
        let n = Node::new(1_u64)
            .with_label("Person")
            .with_label("Employee")
            .with_label("Person"); // duplicate
        assert!(n.has_label("Person"));
        assert!(n.has_label("Employee"));
        assert_eq!(n.labels.len(), 2, "duplicate label should be deduplicated");
    }

    #[test]
    fn builder_adds_properties() {
        let n = Node::new(2_u64)
            .with_property("name", "Alice")
            .with_property("age", 30_i64);
        assert_eq!(
            n.property("name"),
            Some(&PropertyValue::String("Alice".into()))
        );
        assert_eq!(n.property("age"), Some(&PropertyValue::Integer(30)));
        assert_eq!(n.property("missing"), None);
    }

    #[test]
    fn missing_property_vs_null_property() {
        let n = Node::new(3_u64).with_property("x", PropertyValue::Null);
        // Explicitly set to null → Some(Null)
        assert_eq!(n.property("x"), Some(&PropertyValue::Null));
        // Never set → None
        assert_eq!(n.property("y"), None);
    }

    #[test]
    fn node_id_newtype_get() {
        assert_eq!(NodeId(42).get(), 42_u64);
        assert_eq!(NodeId::from(7_u64), NodeId(7));
    }

    #[test]
    fn serde_roundtrip_node() {
        let n = Node::new(5_u64)
            .with_label("Thing")
            .with_property("val", 99_i64);
        let json = serde_json::to_string(&n).expect("serialize");
        let back: Node = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(n, back);
    }
}
