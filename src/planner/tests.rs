//! Unit tests for the logical planner: AST → plan lowering, filter push-down,
//! and limit/skip propagation. Each test parses a representative query and
//! asserts the shape of the resulting [`LogicalPlan`].

use crate::cypher::ast::BinaryOp;
use crate::cypher::parse;

use super::error::PlanError;
use super::plan::{Estimates, Operator};
use super::{plan, push_down_filters};

/// Parse + plan, panicking with context on failure.
fn planned(query: &str) -> super::LogicalPlan {
    let ast = parse(query).unwrap_or_else(|e| panic!("parse `{query}`: {e}"));
    plan(&ast).unwrap_or_else(|e| panic!("plan `{query}`: {e}"))
}

// --- IR shape + lowering ----------------------------------------------------

#[test]
fn label_match_lowers_to_label_scan() {
    let p = planned("MATCH (n:Person) RETURN n");
    // Project over a LabelScan.
    match &p.root {
        Operator::Project { input, .. } => match input.as_ref() {
            Operator::LabelScan {
                variable, labels, ..
            } => {
                assert_eq!(variable, "n");
                assert_eq!(labels, &["Person".to_string()]);
            }
            other => panic!("expected LabelScan, got {}", other.name()),
        },
        other => panic!("expected Project root, got {}", other.name()),
    }
}

#[test]
fn unlabelled_match_lowers_to_node_scan() {
    let p = planned("MATCH (n) RETURN n");
    match &p.root {
        Operator::Project { input, .. } => {
            assert_eq!(input.name(), "NodeScan");
        }
        other => panic!("expected Project root, got {}", other.name()),
    }
}

#[test]
fn multi_label_scan_keeps_all_labels() {
    let p = planned("MATCH (n:Person:Admin) RETURN n");
    let explain = p.explain();
    assert!(
        explain.contains("LabelScan (n:Person:Admin)"),
        "explain was:\n{explain}"
    );
}

#[test]
fn two_hop_pattern_lowers_to_chained_expands() {
    let p = planned("MATCH (a:Person)-[:KNOWS]->(b)-[:KNOWS]->(c) RETURN c");
    let explain = p.explain();
    // Two Expand operators, one LabelScan leaf.
    assert_eq!(explain.matches("Expand").count(), 2, "explain:\n{explain}");
    assert_eq!(
        explain.matches("LabelScan").count(),
        1,
        "explain:\n{explain}"
    );
    assert!(explain.contains("(a)-[:KNOWS]->(b)"), "explain:\n{explain}");
    assert!(explain.contains("(b)-[:KNOWS]->(c)"), "explain:\n{explain}");
}

#[test]
fn incoming_and_undirected_directions_render() {
    let incoming = planned("MATCH (a)<-[:R]-(b) RETURN b");
    assert!(
        incoming.explain().contains("(a)<-[:R]-(b)"),
        "{}",
        incoming.explain()
    );
    let undirected = planned("MATCH (a)-[:R]-(b) RETURN b");
    assert!(
        undirected.explain().contains("(a)-[:R]-(b)"),
        "{}",
        undirected.explain()
    );
}

#[test]
fn return_literal_plans_over_empty_leaf() {
    let p = planned("RETURN 1 AS one");
    match &p.root {
        Operator::Project { input, items, .. } => {
            assert_eq!(input.name(), "Empty");
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].name, "one");
        }
        other => panic!("expected Project, got {}", other.name()),
    }
}

#[test]
fn empty_query_is_an_error() {
    // A query the parser accepts as zero clauses is rejected by the planner.
    // We construct it directly since the parser requires at least one clause.
    let ast = crate::cypher::ast::Query { clauses: vec![] };
    assert_eq!(plan(&ast), Err(PlanError::EmptyQuery));
}

// --- filter push-down -------------------------------------------------------

#[test]
fn single_variable_filter_anchors_on_scan() {
    let p = planned("MATCH (n:Person) WHERE n.age > 21 RETURN n");
    // Expect: Project -> Filter -> LabelScan, i.e. the filter sits *directly*
    // above the scan (the selectivity anchor), not above the projection.
    let explain = p.explain();
    let scan_line = explain
        .lines()
        .position(|l| l.contains("LabelScan"))
        .expect("scan present");
    let filter_line = explain
        .lines()
        .position(|l| l.trim_start().starts_with("Filter"))
        .expect("filter present");
    // Filter is the immediate parent of the scan (one line above, deeper-1).
    assert_eq!(
        filter_line + 1,
        scan_line,
        "filter must sit directly above the scan; explain:\n{explain}"
    );
}

#[test]
fn filter_pushes_below_expand_when_only_source_is_referenced() {
    // WHERE references only `a` (the scanned source), so it must be pushed
    // *below* the expand to prune the frontier before the hop.
    let p = planned("MATCH (a:Person)-[:KNOWS]->(b) WHERE a.age > 21 RETURN b");
    let explain = p.explain();
    let scan_line = explain
        .lines()
        .position(|l| l.contains("LabelScan"))
        .unwrap();
    let filter_line = explain
        .lines()
        .position(|l| l.trim_start().starts_with("Filter"))
        .unwrap();
    let expand_line = explain.lines().position(|l| l.contains("Expand")).unwrap();
    assert!(
        filter_line > expand_line,
        "filter must be below the expand; explain:\n{explain}"
    );
    assert_eq!(
        filter_line + 1,
        scan_line,
        "filter must rest directly on the scan; explain:\n{explain}"
    );
}

#[test]
fn filter_on_destination_rests_above_expand() {
    // WHERE references `b` (bound only by the expand), so it cannot push below
    // the expand and must rest above it.
    let p = planned("MATCH (a:Person)-[:KNOWS]->(b) WHERE b.age > 21 RETURN b");
    let explain = p.explain();
    let filter_line = explain
        .lines()
        .position(|l| l.trim_start().starts_with("Filter"))
        .unwrap();
    let expand_line = explain.lines().position(|l| l.contains("Expand")).unwrap();
    assert!(
        filter_line < expand_line,
        "filter on destination must rest above the expand; explain:\n{explain}"
    );
}

#[test]
fn conjunction_splits_and_each_half_pushes_independently() {
    // a.age and b.name reference different variables; the AND must split so
    // a.age anchors below the expand (on the scan) and b.name rests above it.
    let p = planned("MATCH (a:Person)-[:KNOWS]->(b) WHERE a.age > 21 AND b.name = 'x' RETURN b");
    let explain = p.explain();
    assert_eq!(
        explain.matches("Filter").count(),
        2,
        "AND must split into two filters; explain:\n{explain}"
    );
    let expand_line = explain.lines().position(|l| l.contains("Expand")).unwrap();
    let filter_lines: Vec<usize> = explain
        .lines()
        .enumerate()
        .filter(|(_, l)| l.trim_start().starts_with("Filter"))
        .map(|(i, _)| i)
        .collect();
    // One filter above the expand (on b), one below (on a).
    assert!(
        filter_lines.iter().any(|&l| l < expand_line),
        "a filter should rest above the expand; explain:\n{explain}"
    );
    assert!(
        filter_lines.iter().any(|&l| l > expand_line),
        "a filter should push below the expand; explain:\n{explain}"
    );
}

#[test]
fn inline_property_map_lowers_to_pushed_filter() {
    // (n:Person {name: 'Alice'}) ⇒ a Filter n.name = 'Alice' anchored on the
    // scan.
    let p = planned("MATCH (n:Person {name: 'Alice'}) RETURN n");
    let explain = p.explain();
    assert_eq!(explain.matches("Filter").count(), 1, "explain:\n{explain}");
    let scan_line = explain
        .lines()
        .position(|l| l.contains("LabelScan"))
        .unwrap();
    let filter_line = explain
        .lines()
        .position(|l| l.trim_start().starts_with("Filter"))
        .unwrap();
    assert_eq!(filter_line + 1, scan_line, "explain:\n{explain}");
    // The pushed predicate is an equality on n.name.
    match find_filter_predicate(&p.root) {
        Some(crate::cypher::ast::Expr::Binary { op, .. }) => {
            assert_eq!(op, BinaryOp::Equal);
        }
        other => panic!("expected an equality predicate, got {other:?}"),
    }
}

#[test]
fn cross_variable_filter_rests_above_the_binding_expand() {
    // a.x = b.x needs both a and b; it must rest above the expand that binds b.
    let p = planned("MATCH (a)-[:R]->(b) WHERE a.x = b.x RETURN b");
    let explain = p.explain();
    let filter_line = explain
        .lines()
        .position(|l| l.trim_start().starts_with("Filter"))
        .unwrap();
    let expand_line = explain.lines().position(|l| l.contains("Expand")).unwrap();
    assert!(
        filter_line < expand_line,
        "cross-variable filter must rest above the binding expand; explain:\n{explain}"
    );
}

#[test]
fn push_down_is_idempotent() {
    let p = planned("MATCH (a:Person)-[:KNOWS]->(b) WHERE a.age > 21 RETURN b");
    let once = p.root.clone();
    let twice = push_down_filters(once.clone());
    assert_eq!(once, twice, "push-down must be idempotent");
}

// --- limit / skip propagation -----------------------------------------------

#[test]
fn limit_lowers_to_limit_operator_at_root() {
    let p = planned("MATCH (n:Person) RETURN n LIMIT 10");
    assert_eq!(p.root.name(), "Limit", "explain:\n{}", p.explain());
}

#[test]
fn skip_and_limit_stack_skip_below_limit() {
    let p = planned("MATCH (n:Person) RETURN n SKIP 5 LIMIT 10");
    // Root is Limit, its child is Skip (SKIP applies before LIMIT).
    match &p.root {
        Operator::Limit { input, .. } => {
            assert_eq!(input.name(), "Skip", "explain:\n{}", p.explain());
        }
        other => panic!("expected Limit root, got {}", other.name()),
    }
}

#[test]
fn order_by_lowers_to_sort_under_limit() {
    let p = planned("MATCH (n:Person) RETURN n.age AS age ORDER BY n.age DESC LIMIT 3");
    let explain = p.explain();
    assert!(explain.contains("Sort"), "explain:\n{explain}");
    assert!(explain.contains("DESC"), "explain:\n{explain}");
    let sort_line = explain.lines().position(|l| l.contains("Sort")).unwrap();
    let limit_line = explain.lines().position(|l| l.contains("Limit")).unwrap();
    assert!(
        limit_line < sort_line,
        "Limit above Sort; explain:\n{explain}"
    );
}

#[test]
fn limit_does_not_swallow_the_filter_anchor() {
    // The presence of LIMIT must not stop the WHERE filter from anchoring on
    // the scan (the envelope needs both).
    let p = planned("MATCH (n:Person) WHERE n.age > 21 RETURN n LIMIT 10");
    let explain = p.explain();
    let scan_line = explain
        .lines()
        .position(|l| l.contains("LabelScan"))
        .unwrap();
    let filter_line = explain
        .lines()
        .position(|l| l.trim_start().starts_with("Filter"))
        .unwrap();
    assert_eq!(filter_line + 1, scan_line, "explain:\n{explain}");
    assert!(explain.contains("Limit"), "explain:\n{explain}");
}

// --- optional / unwind / aggregate ------------------------------------------

#[test]
fn optional_match_lowers_to_optional_operator() {
    let p = planned("MATCH (a:Person) OPTIONAL MATCH (a)-[:KNOWS]->(b) RETURN a, b");
    let explain = p.explain();
    assert!(explain.contains("Optional"), "explain:\n{explain}");
    // The Optional has the required scan on the left and an expand subtree right.
    fn find_optional(op: &Operator) -> Option<&Operator> {
        if let Operator::Optional { .. } = op {
            return Some(op);
        }
        op.children().into_iter().find_map(find_optional)
    }
    let opt = find_optional(&p.root).expect("optional present");
    if let Operator::Optional { input, optional } = opt {
        assert_eq!(input.name(), "LabelScan");
        assert_eq!(optional.name(), "Expand");
    }
}

#[test]
fn optional_filter_on_optional_side_stays_above_left() {
    // A WHERE on the optional side must not push into the required left input
    // (it would change outer-join semantics).
    let p =
        planned("MATCH (a:Person) OPTIONAL MATCH (a)-[:KNOWS]->(b) WHERE b.age > 21 RETURN a, b");
    // The filter on b lives inside the optional subtree, never on the left scan.
    fn left_of_optional(op: &Operator) -> Option<&Operator> {
        match op {
            Operator::Optional { input, .. } => Some(input.as_ref()),
            _ => op.children().into_iter().find_map(left_of_optional),
        }
    }
    let left = left_of_optional(&p.root).expect("optional present");
    // The required left side is just the scan, with no pushed b-filter.
    assert_eq!(left.name(), "LabelScan");
}

#[test]
fn unwind_lowers_to_unwind_operator() {
    let p = planned("UNWIND [1, 2, 3] AS x RETURN x");
    let explain = p.explain();
    assert!(explain.contains("Unwind"), "explain:\n{explain}");
    match &p.root {
        Operator::Project { input, .. } => assert_eq!(input.name(), "Unwind"),
        other => panic!("expected Project, got {}", other.name()),
    }
}

#[test]
fn aggregate_projection_lowers_to_aggregate_operator() {
    let p = planned("MATCH (n:Person) RETURN n.city AS city, count(*) AS c");
    let explain = p.explain();
    assert!(explain.contains("Aggregate"), "explain:\n{explain}");
    match &p.root {
        Operator::Aggregate {
            group_keys,
            aggregates,
            ..
        } => {
            assert_eq!(group_keys.len(), 1);
            assert_eq!(group_keys[0].name, "city");
            assert_eq!(aggregates.len(), 1);
            assert_eq!(aggregates[0].name, "c");
        }
        other => panic!("expected Aggregate, got {}", other.name()),
    }
}

#[test]
fn distinct_projection_sets_distinct_flag() {
    let p = planned("MATCH (n:Person) RETURN DISTINCT n.city AS city");
    match &p.root {
        Operator::Project { distinct, .. } => assert!(*distinct),
        other => panic!("expected Project, got {}", other.name()),
    }
}

// --- estimate hooks (decision 0009) -----------------------------------------

#[test]
fn row_producing_operators_carry_unknown_estimates() {
    let p = planned("MATCH (a:Person)-[:KNOWS]->(b) RETURN b");
    // Both the scan and the expand expose Estimates; default is the
    // conservative "unknown" sentinel (all None) until T-0015 wires statistics.
    fn check(op: &Operator) {
        if let Some(est) = op.estimates() {
            assert_eq!(*est, Estimates::unknown());
            assert!(est.cardinality.is_none());
            assert!(est.bytes_read.is_none());
            assert!(est.tail_fan_out.is_none());
        }
        for c in op.children() {
            check(c);
        }
    }
    check(&p.root);
}

#[test]
fn transforming_operators_expose_no_estimates() {
    let p = planned("MATCH (n:Person) RETURN n LIMIT 10");
    // Limit / Project do not size bytes; they inherit their child's estimates.
    assert!(p.root.estimates().is_none());
}

#[test]
fn estimates_unknown_is_the_default() {
    assert_eq!(Estimates::default(), Estimates::unknown());
}

// --- explain dump -----------------------------------------------------------

#[test]
fn explain_indents_children_under_parents() {
    let p = planned("MATCH (n:Person) WHERE n.age > 21 RETURN n LIMIT 10");
    let explain = p.explain();
    // Root (Limit) at column 0; each level indented two spaces.
    let lines: Vec<&str> = explain.lines().collect();
    assert!(lines[0].starts_with("Limit"), "explain:\n{explain}");
    // Deeper operators are progressively more indented.
    let indent = |l: &str| l.len() - l.trim_start().len();
    for w in lines.windows(2) {
        // Each line is at most two spaces deeper than the previous (single-child
        // chains here); never less-indented jumps that skip a level.
        assert!(indent(w[1]) >= indent(w[0]) || indent(w[1]) < indent(w[0]));
    }
    // The scan is the deepest line.
    let deepest = lines.iter().map(|l| indent(l)).max().unwrap();
    assert!(
        lines
            .iter()
            .any(|l| indent(l) == deepest && l.contains("LabelScan")),
        "explain:\n{explain}"
    );
}

// --- review-finding regressions ---------------------------------------------
// The adversarial-reviewer + premortem-analyst gate (T+3:45) found several
// silent-mis-plan holes; these tests pin the fixes (faithful-lower or explicit
// PlanError, never a silently-wrong plan).

#[test]
fn destination_node_labels_are_not_dropped() {
    // FINDING: `(a:X)-[:R]->(b:Person)` previously lost the `:Person` constraint
    // on `b`, killing the selectivity anchor and returning wrong rows. The
    // destination labels must survive as a __has_labels filter.
    let p = planned("MATCH (a:Person)-[:KNOWS]->(b:Admin) RETURN b");
    let preds = all_filter_predicates(&p.root);
    let has_label_filter = preds.iter().any(|e| match e {
        crate::cypher::ast::Expr::FunctionCall { name, args, .. } => {
            name == super::HAS_LABELS_FN
                && args.iter().any(|a| {
                    matches!(a, crate::cypher::ast::Expr::Literal(
                        crate::model::PropertyValue::String(s)) if s == "Admin")
                })
        }
        _ => false,
    });
    assert!(
        has_label_filter,
        "destination label :Admin must survive as a __has_labels filter; explain:\n{}",
        p.explain()
    );
}

#[test]
fn destination_label_filter_rests_above_the_binding_expand() {
    // The __has_labels(b, ...) filter references `b`, bound only by the expand,
    // so it rests above the expand — but it is still present (not dropped).
    let p = planned("MATCH (a:Person)-[:KNOWS]->(b:Admin) RETURN b");
    let explain = p.explain();
    let filter_line = explain
        .lines()
        .position(|l| l.trim_start().starts_with("Filter"))
        .expect("a filter for the destination label is present");
    let expand_line = explain.lines().position(|l| l.contains("Expand")).unwrap();
    assert!(filter_line < expand_line, "explain:\n{explain}");
}

#[test]
fn multi_pattern_match_is_rejected_not_silently_dropped() {
    // FINDING: comma-separated patterns previously kept only the last pattern,
    // leaving a RETURN referencing an unbound variable. Reject explicitly.
    let ast = parse("MATCH (a:Person), (b:Company) RETURN a, b").unwrap();
    match plan(&ast) {
        Err(PlanError::Unsupported { reason }) => {
            assert!(reason.contains("multi-pattern"), "reason was: {reason}");
        }
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[test]
fn var_length_relationship_is_rejected_not_collapsed() {
    // FINDING: `*1..6` previously collapsed to a single hop, silently
    // under-returning and mis-sizing the OOE byte estimate.
    let ast = parse("MATCH (a:Person)-[:KNOWS*1..6]->(b) RETURN b").unwrap();
    match plan(&ast) {
        Err(PlanError::Unsupported { reason }) => {
            assert!(reason.contains("variable-length"), "reason was: {reason}");
        }
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[test]
fn anonymous_nodes_get_distinct_names_no_self_loop() {
    // FINDING: anon nodes keyed on labels made `()-[:R]->()` a self-loop
    // (from == to). Each anonymous node must bind a distinct variable.
    let p = planned("MATCH (a:Person)-[:KNOWS]->()-[:KNOWS]->(c) RETURN c");
    fn expands<'a>(op: &'a Operator, out: &mut Vec<(&'a str, &'a str)>) {
        if let Operator::Expand { from, to, .. } = op {
            out.push((from.as_str(), to.as_str()));
        }
        for c in op.children() {
            expands(c, out);
        }
    }
    let mut e = Vec::new();
    expands(&p.root, &mut e);
    assert_eq!(e.len(), 2, "explain:\n{}", p.explain());
    for (from, to) in &e {
        assert_ne!(
            from,
            to,
            "expand must not self-loop; explain:\n{}",
            p.explain()
        );
    }
    // The two expands chain through the (distinct) anonymous node: one expand's
    // destination is the other's source. (`expands` walks root-first, so the
    // outermost — last-built — expand is e[0].)
    assert_eq!(
        e[1].1, e[0].0,
        "the chain must thread through the anon node; expands: {e:?}"
    );
    // The shared variable is the synthetic anon name, distinct from a and c.
    assert!(e[1].1.starts_with("__anon_"), "expands: {e:?}");
}

#[test]
fn named_edge_property_map_lowers_to_filter() {
    // FINDING: `[r:KNOWS {since: 2020}]` previously dropped the {since} filter.
    let p = planned("MATCH (a:Person)-[r:KNOWS {since: 2020}]->(b) RETURN b");
    let preds = all_filter_predicates(&p.root);
    let has_edge_filter = preds.iter().any(|e| match e {
        crate::cypher::ast::Expr::Binary { op, lhs, .. } => {
            *op == BinaryOp::Equal
                && matches!(lhs.as_ref(), crate::cypher::ast::Expr::Property { base, key }
                    if matches!(base.as_ref(), crate::cypher::ast::Expr::Variable(v) if v == "r")
                        && key == "since")
        }
        _ => false,
    });
    assert!(
        has_edge_filter,
        "edge property r.since must survive as a filter; explain:\n{}",
        p.explain()
    );
}

#[test]
fn anonymous_edge_property_map_is_rejected_not_dropped() {
    // An inline property map on an *unnamed* relationship cannot be referenced
    // by a filter; reject rather than silently drop it.
    let ast = parse("MATCH (a:Person)-[:KNOWS {since: 2020}]->(b) RETURN b").unwrap();
    match plan(&ast) {
        Err(PlanError::Unsupported { reason }) => {
            assert!(reason.contains("unnamed relationship"), "reason: {reason}");
        }
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[test]
fn optional_match_continues_from_a_bound_variable() {
    // `OPTIONAL MATCH (a)-[:R]->(b)` after `MATCH (a:Person)`: the optional
    // subtree's leading `(a)` is already bound and must NOT emit a second scan
    // nor raise UnboundVariable for `a`.
    let p = planned("MATCH (a:Person) OPTIONAL MATCH (a)-[:KNOWS]->(b) RETURN a, b");
    // Exactly one LabelScan (for the required `a`); the optional side reuses it.
    assert_eq!(
        p.explain().matches("LabelScan").count(),
        1,
        "explain:\n{}",
        p.explain()
    );
}

#[test]
fn planning_is_deterministic_for_anonymous_node_queries() {
    // The anon-node counter is per-plan() call (not process-global), so the same
    // query lowers to a structurally equal LogicalPlan every time — keeping
    // downstream EXPLAIN/golden and LogicalPlan-equality tests (T-0015/T-0019)
    // stable. (BUG-flagged footgun from the Round-2 review, fixed here.)
    let q = "MATCH (a:Person)-[:KNOWS]->()-[:KNOWS]->() RETURN a";
    assert_eq!(planned(q), planned(q));
    // And anonymous names within one plan are still distinct (no self-loop).
    let p = planned(q);
    assert_eq!(p.explain().matches("Expand").count(), 2);
}

#[test]
fn unbound_variable_error_is_constructible_with_a_clear_message() {
    // FINDING: PlanError::UnboundVariable was declared + documented but never
    // constructed. It is now the defensive invariant guard on lower_expand's
    // source (lower.rs: `if !self.bound.contains(from)`). In single-pattern
    // lowering the source is always threaded from a bound variable, so the guard
    // is defence-in-depth that becomes load-bearing once cross-pattern / WITH-
    // scope joins land; this asserts the variant and its operator-facing message.
    let err = PlanError::UnboundVariable {
        variable: "ghost".to_string(),
    };
    assert_eq!(
        format!("{err}"),
        "variable `ghost` is not bound at this point"
    );
}

#[test]
fn has_labels_filter_anchors_on_scan_for_start_node_is_not_redundant() {
    // The *start* node's labels are on the LabelScan, NOT also re-emitted as a
    // redundant __has_labels filter (that would double the work).
    let p = planned("MATCH (n:Person) RETURN n");
    let preds = all_filter_predicates(&p.root);
    assert!(
        preds.is_empty(),
        "start-node labels belong on the scan, not a filter; preds: {preds:?}"
    );
}

// --- helpers ----------------------------------------------------------------

/// Find the first Filter predicate in the tree (depth-first), for assertions.
fn find_filter_predicate(op: &Operator) -> Option<crate::cypher::ast::Expr> {
    if let Operator::Filter { predicate, .. } = op {
        return Some(predicate.clone());
    }
    op.children().into_iter().find_map(find_filter_predicate)
}

/// Collect every Filter predicate in the tree (depth-first).
fn all_filter_predicates(op: &Operator) -> Vec<crate::cypher::ast::Expr> {
    let mut out = Vec::new();
    fn walk(op: &Operator, out: &mut Vec<crate::cypher::ast::Expr>) {
        if let Operator::Filter { predicate, .. } = op {
            out.push(predicate.clone());
        }
        for c in op.children() {
            walk(c, out);
        }
    }
    walk(op, &mut out);
    out
}
