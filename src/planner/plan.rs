//! The logical query-plan intermediate representation (Plan IR).
//!
//! A [`LogicalPlan`] is a tree of [`Operator`]s: leaves read rows out of the
//! graph (scans), interior nodes transform the row stream (expand, filter,
//! project, sort, limit). The planner ([`super::plan`]) lowers a parsed
//! [`Query`](crate::cypher::ast::Query) into this IR; the out-of-envelope
//! estimator (T-0015), the index-selection pass (EPIC-005), and the executor
//! (T-0019) all consume it.
//!
//! # Why a separate logical IR
//!
//! The AST mirrors *source syntax* (clauses in textual order, `WHERE` attached
//! to a `MATCH`). The IR mirrors *evaluation*: a bottom-up dataflow where a
//! scan's selectivity anchors the whole plan. Keeping them distinct lets the
//! filter-push-down pass (the selectivity anchor ADR-0001 requires) rewrite the
//! operator tree without touching the parser, and lets the estimator reason over
//! a small, uniform operator vocabulary instead of the full grammar.
//!
//! # Estimate hooks (decision 0009 / ADR-0001 Part 4)
//!
//! Every operator carries an [`Estimates`] block — placeholder cardinality and
//! byte hooks the out-of-envelope detector fills in once the manifest-statistics
//! contract (SPIKE-0004) lands. This task defines the *shape* of those hooks
//! (so the IR is the stable surface T-0015 plans against) without wiring any
//! real statistics: the defaults are the conservative "unknown" sentinel
//! ([`Estimates::unknown`]).

use crate::cypher::ast::{Direction, Expr};

/// The reserved function name used in a [`Filter`](Operator::Filter) predicate
/// to test that a node carries a set of labels.
///
/// A node-label restriction (`(b:Person)`) on a node that is *not* the
/// pattern's scan anchor cannot be a `LabelScan` (the rows already exist); it
/// must be a filter. openCypher's expression grammar has no native label-test
/// node, so the planner lowers it to a synthetic call
/// `__has_labels(var, "Label1", "Label2", …)`. The executor (T-0019) and the
/// out-of-envelope estimator (T-0015) recognise this name; treating it as an
/// ordinary user function would be a bug. The double-underscore prefix keeps it
/// out of the user function namespace.
pub const HAS_LABELS_FN: &str = "__has_labels";

/// A complete logical plan: the root operator. Rows flow from the leaves up to
/// the root.
#[derive(Debug, Clone, PartialEq)]
pub struct LogicalPlan {
    /// The root operator.
    pub root: Operator,
}

impl LogicalPlan {
    /// Wrap a root operator in a plan.
    #[must_use]
    pub fn new(root: Operator) -> Self {
        Self { root }
    }

    /// Render the plan as an indented, `EXPLAIN`-style tree, one operator per
    /// line. Used by tests to assert push-down and operator ordering, and by a
    /// future `EXPLAIN` surface.
    #[must_use]
    pub fn explain(&self) -> String {
        let mut out = String::new();
        self.root.write_explain(&mut out, 0);
        out
    }
}

/// A logical operator: one node in the plan tree.
///
/// Operators own their children directly (boxed), so a [`LogicalPlan`] is a
/// self-contained tree. The vocabulary is intentionally small — it is the
/// surface the estimator and executor share.
#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    /// Scan every node in the graph, binding each to `variable`. The
    /// unanchored leaf — the planner avoids it whenever a label or a
    /// pushed-down predicate can narrow the scan.
    NodeScan {
        /// The variable bound to each scanned node.
        variable: String,
        /// Cardinality / byte estimate hooks for the detector.
        estimates: Estimates,
    },
    /// Scan nodes carrying *all* of `labels`, binding each to `variable`. The
    /// primary selectivity anchor (ADR-0001 §2): a label restriction shrinks the
    /// seed set before any expansion.
    LabelScan {
        /// The variable bound to each scanned node.
        variable: String,
        /// The labels every scanned node must carry (`:A:B` ⇒ `["A", "B"]`).
        labels: Vec<String>,
        /// Cardinality / byte estimate hooks for the detector.
        estimates: Estimates,
    },
    /// Expand one hop from `from` along a relationship to `to`, reading the
    /// source frontier's adjacency lists. The hop phase of ADR-0001's cost
    /// model; `estimates.tail_fan_out` is the `F_tail` the byte budget is sized
    /// against (decision 0009).
    Expand {
        /// The child producing the source frontier.
        input: Box<Operator>,
        /// The already-bound source variable.
        from: String,
        /// The relationship variable, if the pattern named one.
        rel_variable: Option<String>,
        /// Candidate relationship types (`:A|B`); empty means any type.
        rel_types: Vec<String>,
        /// Traversal direction.
        direction: Direction,
        /// The variable bound to the destination node.
        to: String,
        /// Cardinality / byte estimate hooks for the detector.
        estimates: Estimates,
    },
    /// Keep only rows for which `predicate` holds. Filter push-down drives these
    /// as close to the producing scan/expand as the predicate's variables allow.
    Filter {
        /// The child whose rows are filtered.
        input: Box<Operator>,
        /// The boolean predicate.
        predicate: Expr,
    },
    /// Project the input rows to `items` (the `RETURN` / `WITH` column list).
    Project {
        /// The child producing the rows to project.
        input: Box<Operator>,
        /// The output columns, in order.
        items: Vec<ProjectionColumn>,
        /// `true` if `DISTINCT` was requested.
        distinct: bool,
    },
    /// Group + aggregate: the grouping keys plus the aggregating expressions
    /// (e.g. `count(*)`, `sum(n.x)`). Produced when a projection mixes aggregate
    /// and non-aggregate items.
    Aggregate {
        /// The child producing the rows to aggregate.
        input: Box<Operator>,
        /// The non-aggregate grouping-key columns.
        group_keys: Vec<ProjectionColumn>,
        /// The aggregating columns.
        aggregates: Vec<ProjectionColumn>,
    },
    /// Sort rows by the given keys (`ORDER BY`).
    Sort {
        /// The child producing the rows to sort.
        input: Box<Operator>,
        /// The sort keys, in priority order.
        keys: Vec<SortKey>,
    },
    /// Drop the first `count` rows (`SKIP`).
    Skip {
        /// The child producing the rows.
        input: Box<Operator>,
        /// The number of rows to drop.
        count: Box<Expr>,
    },
    /// Keep at most `count` rows (`LIMIT`). The early-termination signal the
    /// envelope (ADR-0001 §2.3) depends on: without it a 6-hop expansion cannot
    /// satisfy the byte budget.
    Limit {
        /// The child producing the rows.
        input: Box<Operator>,
        /// The maximum number of rows to keep.
        count: Box<Expr>,
    },
    /// Left-outer apply for `OPTIONAL MATCH`: every left row is preserved; the
    /// right (optional) subtree's variables are null when it produces no match.
    Optional {
        /// The required left input.
        input: Box<Operator>,
        /// The optional right subtree.
        optional: Box<Operator>,
    },
    /// Unwind a list-valued expression into one row per element (`UNWIND`).
    Unwind {
        /// The child producing the rows (an [`Operator::Empty`] for a leading
        /// `UNWIND`).
        input: Box<Operator>,
        /// The list-valued expression.
        expr: Expr,
        /// The variable bound to each element.
        variable: String,
    },
    /// The empty single-row leaf: a query with no `MATCH`/`UNWIND` to scan from
    /// (e.g. `RETURN 1`) projects over this one unit row.
    Empty,
}

impl Operator {
    /// The immediate child operators, in evaluation order (left input first).
    /// Leaves return an empty slice. Used by the push-down pass and `explain`.
    #[must_use]
    pub fn children(&self) -> Vec<&Operator> {
        match self {
            Operator::NodeScan { .. } | Operator::LabelScan { .. } | Operator::Empty => Vec::new(),
            Operator::Filter { input, .. }
            | Operator::Project { input, .. }
            | Operator::Aggregate { input, .. }
            | Operator::Sort { input, .. }
            | Operator::Skip { input, .. }
            | Operator::Limit { input, .. }
            | Operator::Unwind { input, .. }
            | Operator::Expand { input, .. } => vec![input.as_ref()],
            Operator::Optional { input, optional } => {
                vec![input.as_ref(), optional.as_ref()]
            }
        }
    }

    /// The estimate hooks for this operator, if it carries them. Only the
    /// row-producing operators (scans, expand) size bytes/cardinality; the
    /// transforming operators inherit their child's estimates and so return
    /// `None` here.
    #[must_use]
    pub fn estimates(&self) -> Option<&Estimates> {
        match self {
            Operator::NodeScan { estimates, .. }
            | Operator::LabelScan { estimates, .. }
            | Operator::Expand { estimates, .. } => Some(estimates),
            _ => None,
        }
    }

    /// The short, stable operator name used in `EXPLAIN` output and tests.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Operator::NodeScan { .. } => "NodeScan",
            Operator::LabelScan { .. } => "LabelScan",
            Operator::Expand { .. } => "Expand",
            Operator::Filter { .. } => "Filter",
            Operator::Project { .. } => "Project",
            Operator::Aggregate { .. } => "Aggregate",
            Operator::Sort { .. } => "Sort",
            Operator::Skip { .. } => "Skip",
            Operator::Limit { .. } => "Limit",
            Operator::Optional { .. } => "Optional",
            Operator::Unwind { .. } => "Unwind",
            Operator::Empty => "Empty",
        }
    }

    /// A one-line summary of this operator's distinguishing attributes (the
    /// variable it binds, the predicate it tests, …) for `EXPLAIN`.
    fn summary(&self) -> String {
        match self {
            Operator::NodeScan { variable, .. } => format!("({variable})"),
            Operator::LabelScan {
                variable, labels, ..
            } => {
                format!("({variable}:{})", labels.join(":"))
            }
            Operator::Expand {
                from,
                to,
                rel_types,
                direction,
                ..
            } => {
                let types = if rel_types.is_empty() {
                    String::new()
                } else {
                    format!(":{}", rel_types.join("|"))
                };
                let (l, r) = match direction {
                    Direction::Outgoing => ("-", "->"),
                    Direction::Incoming => ("<-", "-"),
                    Direction::Undirected => ("-", "-"),
                };
                format!("({from}){l}[{types}]{r}({to})")
            }
            Operator::Filter { predicate, .. } => format!("{predicate:?}"),
            Operator::Project {
                items, distinct, ..
            } => {
                let d = if *distinct { "DISTINCT " } else { "" };
                let cols: Vec<&str> = items.iter().map(|c| c.name.as_str()).collect();
                format!("{d}{}", cols.join(", "))
            }
            Operator::Aggregate {
                group_keys,
                aggregates,
                ..
            } => {
                let keys: Vec<&str> = group_keys.iter().map(|c| c.name.as_str()).collect();
                let aggs: Vec<&str> = aggregates.iter().map(|c| c.name.as_str()).collect();
                format!("keys=[{}] aggs=[{}]", keys.join(", "), aggs.join(", "))
            }
            Operator::Sort { keys, .. } => {
                let ks: Vec<String> = keys
                    .iter()
                    .map(|k| format!("{}{}", k.name, if k.descending { " DESC" } else { "" }))
                    .collect();
                ks.join(", ")
            }
            Operator::Skip { count, .. } => format!("{count:?}"),
            Operator::Limit { count, .. } => format!("{count:?}"),
            Operator::Unwind { expr, variable, .. } => {
                format!("{expr:?} AS {variable}")
            }
            Operator::Optional { .. } | Operator::Empty => String::new(),
        }
    }

    /// Recursively render this operator and its children into `out`, indenting
    /// each level by two spaces. Left input first, so the tree reads top-down
    /// in evaluation order.
    fn write_explain(&self, out: &mut String, depth: usize) {
        for _ in 0..depth {
            out.push_str("  ");
        }
        out.push_str(self.name());
        let summary = self.summary();
        if !summary.is_empty() {
            out.push(' ');
            out.push_str(&summary);
        }
        out.push('\n');
        for child in self.children() {
            child.write_explain(out, depth + 1);
        }
    }
}

/// One output column of a [`Operator::Project`] / [`Operator::Aggregate`].
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionColumn {
    /// The output column name (the alias, or the source text of the expression).
    pub name: String,
    /// The expression evaluated to produce the column.
    pub expr: Expr,
}

/// One `ORDER BY` sort key in the IR.
#[derive(Debug, Clone, PartialEq)]
pub struct SortKey {
    /// A display name for the key (the projected column or expression text).
    pub name: String,
    /// The expression sorted on.
    pub expr: Expr,
    /// `true` for descending (`DESC`).
    pub descending: bool,
}

/// Per-operator cardinality and byte estimate hooks.
///
/// These are the inputs ADR-0001 Part 4 / decision 0009 require the
/// out-of-envelope detector (T-0015) to read at plan time. This task defines
/// the **shape** only: the planner stamps every operator with
/// [`Estimates::unknown`], the conservative sentinel that downstream code reads
/// as "no maintained statistic — treat as worst case / reject". Wiring real
/// manifest statistics (SPIKE-0004) into these fields is T-0015's job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Estimates {
    /// Estimated output row cardinality, if known. `None` = unknown (the
    /// detector must treat as the conservative upper bound).
    pub cardinality: Option<u64>,
    /// Estimated bytes this operator reads from the object store, if known.
    pub bytes_read: Option<u64>,
    /// The **tail** (p99 / max) out-degree the byte budget must be sized
    /// against for an expand, per decision 0009 (never the mean). `None` for
    /// non-expanding operators or when the statistic is unmaintained.
    pub tail_fan_out: Option<u64>,
}

impl Estimates {
    /// The conservative "no statistics maintained" sentinel: every field
    /// `None`. The detector reads this as "unknown ⇒ worst case / reject"
    /// (decision 0009, ADR-0001 OOE-5).
    #[must_use]
    pub const fn unknown() -> Self {
        Self {
            cardinality: None,
            bytes_read: None,
            tail_fan_out: None,
        }
    }
}

impl Default for Estimates {
    fn default() -> Self {
        Self::unknown()
    }
}
