//! Filter push-down: the selectivity-anchoring rewrite.
//!
//! ADR-0001's latency envelope closes only when the seed set is small, and the
//! seed set is small only when the most selective node-property predicates are
//! evaluated *before* any expansion (ADR-0001 §2, §2.3). The lowering pass
//! ([`super::lower`]) emits every `WHERE` conjunct as a [`Filter`] stacked at
//! the top of the match subtree; this pass relocates each filter down to the
//! deepest operator whose output already binds all the variables the predicate
//! reads.
//!
//! Concretely: a predicate over a single scanned variable (`n.age > 21`) is
//! pushed to sit *directly above the scan that binds `n`*, so it prunes the
//! frontier at the leaf — anchoring selectivity exactly where the cost model
//! needs it. A predicate spanning two variables (`a.x = b.y`) is pushed to the
//! lowest operator that binds both (the expand that introduces the second).
//!
//! The rewrite is purely structural and order-preserving for relational
//! correctness: a filter is moved below an operator only when that operator
//! neither drops nor renames the variables the predicate needs, and never below
//! the operator that *introduces* a needed variable. Aggregation, projection,
//! and the optional (right) side of an `OPTIONAL MATCH` are push-down barriers.

use std::collections::BTreeSet;

use crate::cypher::ast::Expr;

use super::plan::Operator;

/// Rewrite `op`, relocating each [`Filter`](Operator::Filter) as deep as the
/// predicate's free variables permit. Idempotent: running it twice yields the
/// same tree.
#[must_use]
pub fn push_down_filters(op: Operator) -> Operator {
    // First, recurse so children are already in pushed-down form.
    let op = map_children(op, push_down_filters);

    match op {
        Operator::Filter { input, predicate } => {
            let vars = free_variables(&predicate);
            push_filter_into(*input, predicate, &vars)
        }
        other => other,
    }
}

/// Push `predicate` (with precomputed free `vars`) as deep into `op` as legal,
/// returning the rewritten subtree with the filter inserted at its resting
/// point.
fn push_filter_into(op: Operator, predicate: Expr, vars: &BTreeSet<String>) -> Operator {
    // If this operator does not bind all the predicate's variables, the filter
    // cannot sit below it — place it here, above `op`.
    if !binds_all(&op, vars) {
        return wrap_filter(op, predicate);
    }

    match op {
        // Scans bind their variable; a single-variable predicate over that
        // variable rests directly above the scan — the selectivity anchor.
        Operator::NodeScan { .. } | Operator::LabelScan { .. } => wrap_filter(op, predicate),

        // Expand binds `to` (and the rel var). If the predicate needs only
        // variables the *input* binds, push below the expand so the frontier is
        // pruned before the hop. Otherwise the predicate needs `to`/the rel
        // var, so it must rest above this expand.
        Operator::Expand {
            input,
            from,
            rel_variable,
            rel_types,
            direction,
            to,
            estimates,
        } => {
            if binds_all(&input, vars) {
                let new_input = push_filter_into(*input, predicate, vars);
                Operator::Expand {
                    input: Box::new(new_input),
                    from,
                    rel_variable,
                    rel_types,
                    direction,
                    to,
                    estimates,
                }
            } else {
                let rebuilt = Operator::Expand {
                    input,
                    from,
                    rel_variable,
                    rel_types,
                    direction,
                    to,
                    estimates,
                };
                wrap_filter(rebuilt, predicate)
            }
        }

        // A filter can pass through another filter (commute) and keep pushing.
        Operator::Filter {
            input,
            predicate: inner,
        } => {
            let pushed = push_filter_into(*input, predicate, vars);
            Operator::Filter {
                input: Box::new(pushed),
                predicate: inner,
            }
        }

        // Unwind binds its own variable. If the predicate only needs variables
        // the input binds, push below; else rest above.
        Operator::Unwind {
            input,
            expr,
            variable,
        } => {
            if binds_all(&input, vars) {
                let new_input = push_filter_into(*input, predicate, vars);
                Operator::Unwind {
                    input: Box::new(new_input),
                    expr,
                    variable,
                }
            } else {
                wrap_filter(
                    Operator::Unwind {
                        input,
                        expr,
                        variable,
                    },
                    predicate,
                )
            }
        }

        // Optional (left-outer apply): a filter over the left input's variables
        // may push into the left child. A filter referencing the optional side
        // must stay above (pushing it in would change outer-join semantics).
        Operator::Optional { input, optional } => {
            if binds_all(&input, vars) {
                let new_input = push_filter_into(*input, predicate, vars);
                Operator::Optional {
                    input: Box::new(new_input),
                    optional,
                }
            } else {
                wrap_filter(Operator::Optional { input, optional }, predicate)
            }
        }

        // Projection, aggregation, sort, skip, limit are barriers: pushing a
        // filter below them would change which rows survive. Rest above.
        other => wrap_filter(other, predicate),
    }
}

/// Wrap `op` in a [`Filter`](Operator::Filter).
fn wrap_filter(op: Operator, predicate: Expr) -> Operator {
    Operator::Filter {
        input: Box::new(op),
        predicate,
    }
}

/// The set of variables an operator's output binds (its own bindings plus its
/// children's, stopping at projection/aggregation barriers, which rebind to
/// output columns).
fn bound_variables(op: &Operator) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    collect_bound(op, &mut out);
    out
}

fn collect_bound(op: &Operator, out: &mut BTreeSet<String>) {
    match op {
        Operator::NodeScan { variable, .. } | Operator::LabelScan { variable, .. } => {
            out.insert(variable.clone());
        }
        Operator::Expand {
            input,
            to,
            rel_variable,
            from,
            ..
        } => {
            collect_bound(input, out);
            out.insert(from.clone());
            out.insert(to.clone());
            if let Some(rv) = rel_variable {
                out.insert(rv.clone());
            }
        }
        Operator::Unwind {
            input, variable, ..
        } => {
            collect_bound(input, out);
            out.insert(variable.clone());
        }
        Operator::Filter { input, .. } => collect_bound(input, out),
        Operator::Optional { input, optional } => {
            collect_bound(input, out);
            collect_bound(optional, out);
        }
        Operator::Skip { input, .. } | Operator::Limit { input, .. } => {
            collect_bound(input, out);
        }
        // Projection / aggregation / sort rebind to output column names; those
        // are the variables visible above them.
        Operator::Project { items, .. } => {
            for c in items {
                out.insert(c.name.clone());
            }
        }
        Operator::Aggregate {
            group_keys,
            aggregates,
            ..
        } => {
            for c in group_keys.iter().chain(aggregates) {
                out.insert(c.name.clone());
            }
        }
        Operator::Sort { input, .. } => collect_bound(input, out),
        Operator::Empty => {}
    }
}

/// Whether `op`'s output binds every variable in `vars`.
fn binds_all(op: &Operator, vars: &BTreeSet<String>) -> bool {
    let bound = bound_variables(op);
    vars.iter().all(|v| bound.contains(v))
}

/// The free (referenced) variables of an expression. Property access reads its
/// base variable; bare variable references contribute directly.
pub(super) fn free_variables(expr: &Expr) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    collect_free(expr, &mut out);
    out
}

fn collect_free(expr: &Expr, out: &mut BTreeSet<String>) {
    match expr {
        Expr::Variable(v) => {
            out.insert(v.clone());
        }
        Expr::Property { base, .. } => collect_free(base, out),
        Expr::Index { base, index } => {
            collect_free(base, out);
            collect_free(index, out);
        }
        Expr::Unary { operand, .. } => collect_free(operand, out),
        Expr::Binary { lhs, rhs, .. } => {
            collect_free(lhs, out);
            collect_free(rhs, out);
        }
        Expr::IsNull { operand, .. } => collect_free(operand, out),
        Expr::FunctionCall { args, .. } => {
            for a in args {
                collect_free(a, out);
            }
        }
        Expr::List(items) => {
            for e in items {
                collect_free(e, out);
            }
        }
        Expr::Map(entries) => {
            for (_, e) in entries {
                collect_free(e, out);
            }
        }
        Expr::Literal(_) | Expr::Parameter(_) | Expr::CountStar => {}
    }
}

/// Apply `f` to each child of `op`, returning the rebuilt operator. Used to
/// recurse the push-down rewrite before processing the current node.
fn map_children(op: Operator, f: impl Fn(Operator) -> Operator + Copy) -> Operator {
    match op {
        Operator::Filter { input, predicate } => Operator::Filter {
            input: Box::new(f(*input)),
            predicate,
        },
        Operator::Expand {
            input,
            from,
            rel_variable,
            rel_types,
            direction,
            to,
            estimates,
        } => Operator::Expand {
            input: Box::new(f(*input)),
            from,
            rel_variable,
            rel_types,
            direction,
            to,
            estimates,
        },
        Operator::Project {
            input,
            items,
            distinct,
        } => Operator::Project {
            input: Box::new(f(*input)),
            items,
            distinct,
        },
        Operator::Aggregate {
            input,
            group_keys,
            aggregates,
        } => Operator::Aggregate {
            input: Box::new(f(*input)),
            group_keys,
            aggregates,
        },
        Operator::Sort { input, keys } => Operator::Sort {
            input: Box::new(f(*input)),
            keys,
        },
        Operator::Skip { input, count } => Operator::Skip {
            input: Box::new(f(*input)),
            count,
        },
        Operator::Limit { input, count } => Operator::Limit {
            input: Box::new(f(*input)),
            count,
        },
        Operator::Unwind {
            input,
            expr,
            variable,
        } => Operator::Unwind {
            input: Box::new(f(*input)),
            expr,
            variable,
        },
        Operator::Optional { input, optional } => Operator::Optional {
            input: Box::new(f(*input)),
            optional: Box::new(f(*optional)),
        },
        leaf @ (Operator::NodeScan { .. } | Operator::LabelScan { .. } | Operator::Empty) => leaf,
    }
}
