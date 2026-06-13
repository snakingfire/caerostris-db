//! The logical property-graph data model.
//!
//! This is the in-memory, format-independent representation every layer shares:
//! the storage writer/reader serialises *to and from* it, the planner reasons
//! *over* it, the TCK adapter compares results *against* it, and the Python
//! bindings hand it *out* as native objects. It is deliberately decoupled from
//! the on-object byte format (SPIKE-0003) so it can land — and stay stable —
//! before the format spec is ratified (board item T-0006).
//!
//! # The pieces
//!
//! - [`PropertyValue`] — the openCypher value type system (null, boolean,
//!   `i64` integer, `f64` float, string, list, map), with openCypher value
//!   equality ([`PropertyValue::cypher_equal`]) and orderability
//!   ([`PropertyValue::cypher_order`]).
//! - [`Node`] — a vertex: a [`NodeId`], a set of labels, a property map.
//! - [`Edge`] — a directed, typed relationship between two nodes, with its own
//!   property map and an [`EdgeId`].
//! - [`Schema`] — a catalog of the known label / rel-type / property-key names,
//!   the registry the planner statistics are keyed by (decision 0009).
//!
//! Every type is [`Clone`] and serde-(de)serialisable so downstream layers can
//! round-trip the model in tests without depending on the on-object format.
//!
//! # Properties
//!
//! Both nodes and edges carry a [`Properties`] map — string keys to
//! [`PropertyValue`]s, backed by a [`BTreeMap`](std::collections::BTreeMap) so
//! iteration and serialisation order are deterministic.

mod edge;
mod node;
mod schema;
mod value;

use std::collections::BTreeMap;

pub use edge::{Edge, EdgeId};
pub use node::{Node, NodeId};
pub use schema::Schema;
pub use value::PropertyValue;

/// A property map: string keys to [`PropertyValue`]s, deterministically
/// ordered. The property bag carried by both [`Node`] and [`Edge`].
pub type Properties = BTreeMap<String, PropertyValue>;
