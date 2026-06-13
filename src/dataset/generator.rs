//! The synthetic property-graph generator.
//!
//! Produces a directed, typed, labelled property graph of a configurable size
//! with a **power-law in-degree distribution** (so a handful of "super-nodes"
//! absorb a large share of edges — exactly the tail-fan-out case the
//! latency-envelope work, SPIKE-0004, must stress). The output is deterministic
//! given a seed and is streamed node-by-node / edge-by-edge so a 1M-node /
//! 10M-edge graph (or larger) never has to be fully materialised in memory.
//!
//! # Why power-law, and how
//!
//! Real graphs are not uniform: a few nodes have enormous degree. We reproduce
//! that with **rank-based Zipf target sampling**. Each node `i` is assigned an
//! attractiveness weight `w(i) ∝ 1 / (rank(i) + 1)^exponent`, where `rank(i)` is
//! a seed-dependent permutation of the node index (so the hubs are not always
//! node 0). Edge targets are drawn by inverse-CDF sampling over the cumulative
//! weight table; sources are drawn uniformly. The result: target in-degree
//! follows a power law with a heavy tail (super-nodes), while the table costs
//! `O(node_count)` memory, *not* `O(edge_count)` — so edge generation stays a
//! constant-memory stream.
//!
//! The generator only depends on the logical [`model`](crate::model) types, so
//! it can feed the in-memory engine, the (future) storage writers, or the
//! portable JSONL form ([`super::io`]) interchangeably.

use crate::model::{Edge, Node, PropertyValue, Schema};

use super::rng::SplitMix64;

/// The fixed label vocabulary the generator draws from. Small and stable so the
/// per-label statistics the planner keys on (decision 0009) are meaningful.
const LABELS: &[&str] = &["Person", "Company", "Place", "Product", "Topic"];

/// The fixed relationship-type vocabulary.
const REL_TYPES: &[&str] = &["KNOWS", "FOLLOWS", "WORKS_AT", "LOCATED_IN", "TAGGED"];

/// A small, low-cardinality text property domain (good for *selective* filters —
/// the latency envelope anchors on a selective node-property predicate).
const COUNTRIES: &[&str] = &[
    "Sweden", "Norway", "Japan", "Brazil", "Kenya", "Canada", "India", "Chile",
];

/// Configuration for one generation run.
///
/// [`GenConfig::default`] is the headline **1M-node / 10M-edge** graph (board
/// item T-0035). All fields are public so callers (the CLI, benches, tests) can
/// dial size, shape, and seed.
#[derive(Debug, Clone, PartialEq)]
pub struct GenConfig {
    /// Number of nodes to emit.
    pub node_count: u64,
    /// Number of edges to emit.
    pub edge_count: u64,
    /// The seed: identical seeds + identical config ⇒ identical graphs.
    pub seed: u64,
    /// The Zipf exponent controlling tail heaviness. Larger ⇒ heavier tail
    /// (more extreme super-nodes). Must be `> 0.0`.
    pub zipf_exponent: f64,
}

impl Default for GenConfig {
    fn default() -> Self {
        GenConfig {
            node_count: 1_000_000,
            edge_count: 10_000_000,
            seed: 0,
            zipf_exponent: 1.0,
        }
    }
}

impl GenConfig {
    /// A tiny graph (`nodes` nodes, `edges` edges) with the given seed — for
    /// fast unit/determinism tests and the committed sample.
    #[must_use]
    pub fn small(node_count: u64, edge_count: u64, seed: u64) -> Self {
        GenConfig {
            node_count,
            edge_count,
            seed,
            zipf_exponent: 1.0,
        }
    }
}

/// A configured generator. Cheap to construct; the `O(node_count)` cumulative
/// weight table is built lazily on the first call to [`Generator::edges`].
#[derive(Debug, Clone)]
pub struct Generator {
    config: GenConfig,
}

impl Generator {
    /// A generator for `config`.
    #[must_use]
    pub fn new(config: GenConfig) -> Self {
        Generator { config }
    }

    /// The configuration this generator was built from.
    #[must_use]
    pub fn config(&self) -> &GenConfig {
        &self.config
    }

    /// The catalog of every label / rel-type / property-key name the generator
    /// can emit. Stable regardless of seed or size, so downstream layers can
    /// register the schema up front.
    #[must_use]
    pub fn schema(&self) -> Schema {
        let mut s = Schema::new();
        for l in LABELS {
            s.register_label(*l);
        }
        for r in REL_TYPES {
            s.register_rel_type(*r);
        }
        for k in ["name", "bio", "country", "age"] {
            s.register_property_key(k);
        }
        for k in ["weight", "rank"] {
            s.register_property_key(k);
        }
        s
    }

    /// A streaming iterator over exactly [`GenConfig::node_count`] nodes.
    ///
    /// Node ids are `0..node_count`. Each node carries one or two labels and the
    /// text properties `name`, `bio`, `country` plus an integer `age`. The
    /// stream is deterministic and depends only on the config + seed.
    #[must_use]
    pub fn nodes(&self) -> NodeIter {
        NodeIter {
            // Domain-separated sub-seed so node and edge streams are independent.
            rng: SplitMix64::new(self.config.seed ^ 0x4E4F_4445_5345_4544),
            next: 0,
            count: self.config.node_count,
        }
    }

    /// A streaming iterator over exactly [`GenConfig::edge_count`] edges.
    ///
    /// Edge ids are `0..edge_count`. Sources are uniform over the node set;
    /// targets are drawn by Zipf rank sampling so the in-degree distribution is
    /// power-law with super-nodes. Self-loops are avoided (re-rolled).
    ///
    /// # Panics
    ///
    /// Panics if `node_count == 0` while `edge_count > 0` (no nodes to connect).
    #[must_use]
    pub fn edges(&self) -> EdgeIter {
        let n = self.config.node_count;
        assert!(
            n > 0 || self.config.edge_count == 0,
            "cannot generate edges over an empty node set"
        );
        let weights = CumulativeWeights::build(&self.config);
        EdgeIter {
            rng: SplitMix64::new(self.config.seed ^ 0x4544_4745_5F47_454E),
            next: 0,
            count: self.config.edge_count,
            node_count: n,
            weights,
        }
    }
}

/// The cumulative attractiveness table used for Zipf target sampling.
///
/// `cumulative[i]` is the sum of weights of nodes ranked `0..=i`. A target is
/// drawn by picking a uniform point in `[0, total)` and binary-searching for the
/// rank that contains it, then mapping that rank back to a node id through a
/// seed-dependent permutation so the hubs are spread across the id space.
#[derive(Debug, Clone)]
struct CumulativeWeights {
    /// Prefix sums of the per-rank weights (length = node_count).
    cumulative: Vec<f64>,
    /// `permutation[rank]` = the node id assigned that rank (so rank 0, the
    /// heaviest hub, is a pseudo-random node rather than always node 0).
    permutation: Vec<u64>,
}

impl CumulativeWeights {
    fn build(config: &GenConfig) -> Self {
        let n = config.node_count;
        let mut cumulative = Vec::with_capacity(n as usize);
        let mut acc = 0.0f64;
        for rank in 0..n {
            #[allow(clippy::cast_precision_loss)]
            let r = rank as f64;
            let w = 1.0 / (r + 1.0).powf(config.zipf_exponent);
            acc += w;
            cumulative.push(acc);
        }

        // A deterministic permutation of node ids via a Fisher–Yates shuffle so
        // the heaviest ranks are not always the lowest ids.
        let mut permutation: Vec<u64> = (0..n).collect();
        let mut perm_rng = SplitMix64::new(config.seed ^ 0x5045_524D_5554_4154);
        for i in (1..permutation.len()).rev() {
            let j = perm_rng.below((i + 1) as u64) as usize;
            permutation.swap(i, j);
        }

        CumulativeWeights {
            cumulative,
            permutation,
        }
    }

    /// The total weight (the last prefix sum), or `0.0` if empty.
    fn total(&self) -> f64 {
        self.cumulative.last().copied().unwrap_or(0.0)
    }

    /// Draw a target node id by inverse-CDF sampling.
    fn sample(&self, rng: &mut SplitMix64) -> u64 {
        debug_assert!(!self.cumulative.is_empty());
        let point = rng.unit_f64() * self.total();
        // First rank whose prefix sum is strictly greater than `point`.
        let rank = self.cumulative.partition_point(|&c| c <= point);
        let rank = rank.min(self.cumulative.len() - 1);
        self.permutation[rank]
    }
}

/// A streaming iterator over generated [`Node`]s. See [`Generator::nodes`].
#[derive(Debug, Clone)]
pub struct NodeIter {
    rng: SplitMix64,
    next: u64,
    count: u64,
}

impl Iterator for NodeIter {
    type Item = Node;

    fn next(&mut self) -> Option<Node> {
        if self.next >= self.count {
            return None;
        }
        let id = self.next;
        self.next += 1;
        Some(build_node(id, &mut self.rng))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.count - self.next) as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for NodeIter {}

/// A streaming iterator over generated [`Edge`]s. See [`Generator::edges`].
#[derive(Debug, Clone)]
pub struct EdgeIter {
    rng: SplitMix64,
    next: u64,
    count: u64,
    node_count: u64,
    weights: CumulativeWeights,
}

impl Iterator for EdgeIter {
    type Item = Edge;

    fn next(&mut self) -> Option<Edge> {
        if self.next >= self.count {
            return None;
        }
        let id = self.next;
        self.next += 1;

        let source = self.rng.below(self.node_count);
        // Draw a target; avoid self-loops by re-rolling a bounded number of
        // times, then fall back to a neighbour id so we never spin forever on a
        // 1-node graph.
        let mut target = self.weights.sample(&mut self.rng);
        let mut tries = 0;
        while target == source && tries < 4 && self.node_count > 1 {
            target = self.weights.sample(&mut self.rng);
            tries += 1;
        }
        if target == source && self.node_count > 1 {
            target = (source + 1) % self.node_count;
        }

        Some(build_edge(id, source, target, &mut self.rng))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.count - self.next) as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for EdgeIter {}

/// Build one node deterministically from its id and the node-stream RNG.
fn build_node(id: u64, rng: &mut SplitMix64) -> Node {
    let primary = LABELS[(rng.below(LABELS.len() as u64)) as usize];
    let mut node = Node::new(id).with_label(primary);
    // ~25% of nodes carry a second label (multi-label nodes exercise label-set
    // matching).
    if rng.below(4) == 0 {
        let secondary = LABELS[(rng.below(LABELS.len() as u64)) as usize];
        node = node.with_label(secondary);
    }
    let country = COUNTRIES[(rng.below(COUNTRIES.len() as u64)) as usize];
    #[allow(clippy::cast_possible_wrap)]
    let age = 18 + (rng.below(72) as i64);
    node.with_property("name", format!("{primary}-{id}"))
        .with_property("bio", synth_bio(id, rng))
        .with_property("country", country)
        .with_property("age", PropertyValue::Integer(age))
}

/// Build one edge deterministically from its id, endpoints, and edge-stream RNG.
fn build_edge(id: u64, source: u64, target: u64, rng: &mut SplitMix64) -> Edge {
    let rel = REL_TYPES[(rng.below(REL_TYPES.len() as u64)) as usize];
    let weight = quantize6(rng.unit_f64());
    Edge::new(id, rel, source, target)
        .with_property("weight", PropertyValue::Float(weight))
        .with_property("rank", PropertyValue::Integer(1 + (rng.below(5) as i64)))
}

/// Round a unit-interval float to 6 decimal places.
///
/// The portable JSONL form (`super::io`) serialises floats as text. serde_json's
/// shortest-round-trippable writer can, for some full-precision `f64`s, emit a
/// decimal that parses back one ULP away — which would make a graph read back
/// *not bit-equal* to the one generated, breaking the reproducibility contract
/// (the committed sample must round-trip exactly). Quantising the synthetic
/// weight to 6 decimals gives it a short decimal representation that JSON
/// round-trips losslessly, while still exercising float-valued properties.
fn quantize6(x: f64) -> f64 {
    (x * 1_000_000.0).round() / 1_000_000.0
}

/// A short, deterministic synthetic "bio" text (no external/PII data).
fn synth_bio(id: u64, rng: &mut SplitMix64) -> String {
    const ADJ: &[&str] = &["curious", "diligent", "quiet", "bold", "wry", "steady"];
    const NOUN: &[&str] = &["builder", "traveller", "writer", "gardener", "tinkerer"];
    let a = ADJ[(rng.below(ADJ.len() as u64)) as usize];
    let n = NOUN[(rng.below(NOUN.len() as u64)) as usize];
    format!("A {a} {n} (#{id}).")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn default_config_is_headline_size() {
        let c = GenConfig::default();
        assert_eq!(c.node_count, 1_000_000);
        assert_eq!(c.edge_count, 10_000_000);
    }

    #[test]
    fn emits_exactly_the_configured_counts() {
        let g = Generator::new(GenConfig::small(500, 2_000, 7));
        assert_eq!(g.nodes().count(), 500);
        assert_eq!(g.edges().count(), 2_000);
    }

    #[test]
    fn node_ids_are_dense_and_in_order() {
        let g = Generator::new(GenConfig::small(100, 0, 1));
        for (i, node) in g.nodes().enumerate() {
            assert_eq!(node.id.get(), i as u64);
        }
    }

    #[test]
    fn nodes_have_labels_and_text_properties() {
        let g = Generator::new(GenConfig::small(200, 0, 3));
        for node in g.nodes() {
            assert!(!node.labels.is_empty(), "every node must carry a label");
            // text property `name` is a String
            assert!(matches!(
                node.property("name"),
                Some(PropertyValue::String(_))
            ));
            assert!(matches!(
                node.property("bio"),
                Some(PropertyValue::String(_))
            ));
            assert!(matches!(
                node.property("country"),
                Some(PropertyValue::String(_))
            ));
            assert!(matches!(
                node.property("age"),
                Some(PropertyValue::Integer(_))
            ));
        }
    }

    #[test]
    fn edges_are_directed_typed_and_in_range() {
        let n = 300u64;
        let g = Generator::new(GenConfig::small(n, 1_500, 11));
        let mut seen_types = std::collections::BTreeSet::new();
        for edge in g.edges() {
            assert!(edge.source.get() < n);
            assert!(edge.target.get() < n);
            assert!(!edge.rel_type.is_empty());
            assert!(matches!(
                edge.property("weight"),
                Some(PropertyValue::Float(_))
            ));
            seen_types.insert(edge.rel_type.clone());
        }
        // Over 1500 edges we expect several distinct rel types.
        assert!(seen_types.len() >= 2, "expected a variety of rel types");
    }

    #[test]
    fn no_self_loops_when_multiple_nodes() {
        let g = Generator::new(GenConfig::small(50, 1_000, 5));
        for edge in g.edges() {
            assert_ne!(edge.source, edge.target, "self-loop emitted");
        }
    }

    #[test]
    fn generation_is_deterministic_given_a_seed() {
        let cfg = GenConfig::small(250, 1_000, 2024);
        let a_nodes: Vec<Node> = Generator::new(cfg.clone()).nodes().collect();
        let b_nodes: Vec<Node> = Generator::new(cfg.clone()).nodes().collect();
        assert_eq!(a_nodes, b_nodes, "node stream not reproducible");

        let a_edges: Vec<Edge> = Generator::new(cfg.clone()).edges().collect();
        let b_edges: Vec<Edge> = Generator::new(cfg).edges().collect();
        assert_eq!(a_edges, b_edges, "edge stream not reproducible");
    }

    #[test]
    fn different_seeds_produce_different_graphs() {
        let a: Vec<Node> = Generator::new(GenConfig::small(200, 0, 1))
            .nodes()
            .collect();
        let b: Vec<Node> = Generator::new(GenConfig::small(200, 0, 2))
            .nodes()
            .collect();
        assert_ne!(a, b, "distinct seeds should produce distinct graphs");
    }

    #[test]
    fn degree_distribution_is_power_law_with_super_nodes() {
        // Build the in-degree histogram and assert a heavy tail: the single most
        // connected node should absorb a large multiple of the mean degree (a
        // uniform-random graph would not), and a long tail of low-degree nodes
        // should exist.
        let n = 2_000u64;
        let m = 40_000u64;
        let g = Generator::new(GenConfig::small(n, m, 42));
        let mut indeg: BTreeMap<u64, u64> = BTreeMap::new();
        for edge in g.edges() {
            *indeg.entry(edge.target.get()).or_default() += 1;
        }
        let max_indeg = indeg.values().copied().max().unwrap_or(0);
        #[allow(clippy::cast_precision_loss)]
        let mean = m as f64 / n as f64; // = 20
        #[allow(clippy::cast_precision_loss)]
        let max_f = max_indeg as f64;
        assert!(
            max_f > mean * 10.0,
            "expected a super-node with in-degree ≫ mean ({mean}); got max {max_indeg}"
        );
        // A power law has many low-degree nodes: at least 30% of nodes should
        // have in-degree ≤ mean/4.
        let low = (0..n)
            .filter(|id| indeg.get(id).copied().unwrap_or(0) as f64 <= mean / 4.0)
            .count();
        #[allow(clippy::cast_precision_loss)]
        let low_frac = low as f64 / n as f64;
        assert!(
            low_frac > 0.30,
            "expected a long low-degree tail; only {low_frac:.2} of nodes are low-degree"
        );
    }

    #[test]
    fn schema_lists_all_emitted_names() {
        let g = Generator::new(GenConfig::small(100, 200, 1));
        let schema = g.schema();
        // Every label/type/key actually emitted must be in the declared schema.
        for node in g.nodes() {
            for l in &node.labels {
                assert!(schema.knows_label(l), "label {l} missing from schema");
            }
            for k in node.properties.keys() {
                assert!(schema.knows_property_key(k), "key {k} missing from schema");
            }
        }
        for edge in g.edges() {
            assert!(
                schema.knows_rel_type(&edge.rel_type),
                "rel type {} missing from schema",
                edge.rel_type
            );
        }
    }

    #[test]
    fn empty_graph_is_allowed() {
        let g = Generator::new(GenConfig::small(0, 0, 1));
        assert_eq!(g.nodes().count(), 0);
        assert_eq!(g.edges().count(), 0);
    }

    #[test]
    fn single_node_graph_avoids_infinite_self_loop_search() {
        // 1 node, several edges: self-loops are unavoidable, but generation must
        // terminate and stay in range.
        let g = Generator::new(GenConfig::small(1, 5, 1));
        let edges: Vec<Edge> = g.edges().collect();
        assert_eq!(edges.len(), 5);
        for e in edges {
            assert_eq!(e.source.get(), 0);
            assert_eq!(e.target.get(), 0);
        }
    }
}
