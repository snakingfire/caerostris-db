//! A lightweight schema/catalog: the known labels, relationship types, and
//! property keys of a graph.
//!
//! openCypher is *schema-optional* — nodes need not declare labels and
//! properties up front — so this is a **catalog of what has been seen**, not a
//! constraint system. Its job is to give every layer a single, stable set of
//! the label/rel-type/property-key *names* in the graph.
//!
//! It exists now because the planner's out-of-envelope cost model
//! (`.project/decisions/0009-planner-stats-and-tail-fanout-bound.md`) keys its
//! maintained statistics — per-label node counts, per-rel-type degree
//! distributions, per-property selectivity — by exactly these three name
//! spaces. The *statistics themselves* live in the storage manifest
//! (SPIKE-0004); this catalog is only the **name registry** they are keyed by,
//! kept deliberately free of byte-layout and statistics concerns so it can land
//! ahead of the storage-format spec.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{Edge, Node};

/// The known label / relationship-type / property-key names of a graph.
///
/// All three name sets are [`BTreeSet`]s: deduplicated and deterministically
/// ordered, so a serialised catalog is stable regardless of the order names
/// were registered. The catalog is *additive* — names are observed and
/// registered, never removed by the model (GC of unused names is a storage
/// concern, not a logical-model one).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schema {
    labels: BTreeSet<String>,
    rel_types: BTreeSet<String>,
    property_keys: BTreeSet<String>,
}

impl Schema {
    /// An empty catalog.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a label name. Returns `true` if it was newly added.
    pub fn register_label(&mut self, label: impl Into<String>) -> bool {
        self.labels.insert(label.into())
    }

    /// Register a relationship-type name. Returns `true` if newly added.
    pub fn register_rel_type(&mut self, rel_type: impl Into<String>) -> bool {
        self.rel_types.insert(rel_type.into())
    }

    /// Register a property-key name. Returns `true` if newly added.
    pub fn register_property_key(&mut self, key: impl Into<String>) -> bool {
        self.property_keys.insert(key.into())
    }

    /// Observe a node: register its labels and all its property keys.
    pub fn observe_node(&mut self, node: &Node) {
        for label in &node.labels {
            self.labels.insert(label.clone());
        }
        for key in node.properties.keys() {
            self.property_keys.insert(key.clone());
        }
    }

    /// Observe an edge: register its relationship type and all its property
    /// keys.
    pub fn observe_edge(&mut self, edge: &Edge) {
        self.rel_types.insert(edge.rel_type.clone());
        for key in edge.properties.keys() {
            self.property_keys.insert(key.clone());
        }
    }

    /// `true` if `label` is a known label.
    #[must_use]
    pub fn knows_label(&self, label: &str) -> bool {
        self.labels.contains(label)
    }

    /// `true` if `rel_type` is a known relationship type.
    #[must_use]
    pub fn knows_rel_type(&self, rel_type: &str) -> bool {
        self.rel_types.contains(rel_type)
    }

    /// `true` if `key` is a known property key.
    #[must_use]
    pub fn knows_property_key(&self, key: &str) -> bool {
        self.property_keys.contains(key)
    }

    /// The known labels, in sorted order.
    #[must_use]
    pub fn labels(&self) -> &BTreeSet<String> {
        &self.labels
    }

    /// The known relationship types, in sorted order.
    #[must_use]
    pub fn rel_types(&self) -> &BTreeSet<String> {
        &self.rel_types
    }

    /// The known property keys, in sorted order.
    #[must_use]
    pub fn property_keys(&self) -> &BTreeSet<String> {
        &self.property_keys
    }
}

// ---- Unit tests -------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, Node, PropertyValue};

    #[test]
    fn new_schema_is_empty() {
        let s = Schema::new();
        assert!(s.labels().is_empty());
        assert!(s.rel_types().is_empty());
        assert!(s.property_keys().is_empty());
    }

    #[test]
    fn register_label_deduplicates() {
        let mut s = Schema::new();
        assert!(s.register_label("Person"));
        assert!(!s.register_label("Person")); // second registration returns false
        assert_eq!(s.labels().len(), 1);
        assert!(s.knows_label("Person"));
        assert!(!s.knows_label("Animal"));
    }

    #[test]
    fn register_rel_type() {
        let mut s = Schema::new();
        assert!(s.register_rel_type("KNOWS"));
        assert!(!s.register_rel_type("KNOWS"));
        assert!(s.knows_rel_type("KNOWS"));
        assert!(!s.knows_rel_type("LIKES"));
    }

    #[test]
    fn register_property_key() {
        let mut s = Schema::new();
        assert!(s.register_property_key("name"));
        assert!(!s.register_property_key("name"));
        assert!(s.knows_property_key("name"));
        assert!(!s.knows_property_key("age"));
    }

    #[test]
    fn observe_node_registers_labels_and_keys() {
        let mut s = Schema::new();
        let n = Node::new(1_u64)
            .with_label("Person")
            .with_label("Employee")
            .with_property("name", "Alice")
            .with_property("age", 30_i64);
        s.observe_node(&n);
        assert!(s.knows_label("Person"));
        assert!(s.knows_label("Employee"));
        assert!(s.knows_property_key("name"));
        assert!(s.knows_property_key("age"));
        assert!(!s.knows_rel_type("KNOWS"));
    }

    #[test]
    fn observe_edge_registers_rel_type_and_keys() {
        let mut s = Schema::new();
        let e = Edge::new(1_u64, "KNOWS", 1_u64, 2_u64).with_property("since", "2020".to_string());
        s.observe_edge(&e);
        assert!(s.knows_rel_type("KNOWS"));
        assert!(s.knows_property_key("since"));
        assert!(!s.knows_label("Person"));
    }

    #[test]
    fn serde_roundtrip_schema() {
        let mut s = Schema::new();
        s.register_label("Thing");
        s.register_rel_type("HAS");
        s.register_property_key("color");
        let json = serde_json::to_string(&s).expect("serialize");
        let back: Schema = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(s, back);
    }

    #[test]
    fn observe_node_with_null_property_registers_key() {
        let mut s = Schema::new();
        let n = Node::new(1_u64).with_property("x", PropertyValue::Null);
        s.observe_node(&n);
        // The key "x" should be registered even though the value is null
        assert!(s.knows_property_key("x"));
    }
}
