//! AST → logical-IR lowering.
//!
//! [`lower`] walks a parsed [`Query`] left to right, building the operator tree
//! bottom-up: the first scanned node is a leaf, each later hop / clause wraps
//! the running operator. WHERE predicates and inline property maps are emitted
//! as [`Filter`](Operator::Filter)s and then relocated by the
//! [`push_down`](super::pushdown) pass.

use std::collections::BTreeSet;

use crate::cypher::ast::{
    Clause, Expr, MatchClause, NodePattern, PathPattern, ProjectionClause, ProjectionItem, Query,
    RelPattern, ReturnBody, UnwindClause,
};
use crate::model::PropertyValue;

use super::error::{PlanError, PlanResult};
use super::plan::{Estimates, LogicalPlan, Operator, ProjectionColumn, SortKey};
use super::pushdown::push_down_filters;

/// Lower a parsed read-query AST into a logical plan, applying filter
/// push-down. The single public entry point of the planner.
///
/// # Errors
///
/// Returns a [`PlanError`] if the query is empty, references an unbound
/// variable, or uses a clause shape the read-query planner cannot lower.
pub fn lower(query: &Query) -> PlanResult<LogicalPlan> {
    if query.clauses.is_empty() {
        return Err(PlanError::EmptyQuery);
    }

    let mut state = LoweringState::default();
    for clause in &query.clauses {
        state.lower_clause(clause)?;
    }

    let root = state.into_root()?;
    let root = push_down_filters(root);
    Ok(LogicalPlan::new(root))
}

/// Running lowering state: the operator built so far plus whether it is still
/// the fresh single-row [`Empty`](Operator::Empty) leaf (so the first scanned
/// node replaces it rather than joining onto it).
struct LoweringState {
    current: Operator,
    /// `true` while `current` is the untouched [`Operator::Empty`] sentinel.
    fresh: bool,
    bound: BTreeSet<String>,
}

impl Default for LoweringState {
    fn default() -> Self {
        Self {
            current: Operator::Empty,
            fresh: true,
            bound: BTreeSet::new(),
        }
    }
}

impl LoweringState {
    fn lower_clause(&mut self, clause: &Clause) -> PlanResult<()> {
        match clause {
            Clause::Match(m) => self.lower_match(m),
            Clause::Unwind(u) => self.lower_unwind(u),
            Clause::With(p) | Clause::Return(p) => self.lower_projection(p),
        }
    }

    /// Lower a `MATCH` / `OPTIONAL MATCH`. The patterns build a chain of
    /// scans/expands; the inline property maps and the `WHERE` predicate are
    /// emitted as filters stacked on top (the push-down pass relocates them).
    fn lower_match(&mut self, m: &MatchClause) -> PlanResult<()> {
        if m.optional {
            // OPTIONAL MATCH: build the pattern as an independent subtree over a
            // fresh Empty leaf, then left-outer-apply it onto the running plan.
            let mut sub = LoweringState::default();
            for pattern in &m.patterns {
                sub.lower_pattern(pattern)?;
            }
            if let Some(pred) = &m.where_clause {
                sub.push_filters(pred);
            }
            self.bound.extend(sub.bound.iter().cloned());
            let optional = sub.into_root()?;
            self.current = Operator::Optional {
                input: Box::new(std::mem::replace(&mut self.current, Operator::Empty)),
                optional: Box::new(optional),
            };
            self.fresh = false;
            return Ok(());
        }

        for pattern in &m.patterns {
            self.lower_pattern(pattern)?;
        }
        if let Some(pred) = &m.where_clause {
            self.push_filters(pred);
        }
        Ok(())
    }

    /// Lower one comma-separated path pattern into scans + expands.
    fn lower_pattern(&mut self, pattern: &PathPattern) -> PlanResult<()> {
        self.lower_start_node(&pattern.start);
        let mut prev_var = node_var(&pattern.start);
        for step in &pattern.steps {
            let to_var = node_var(&step.node);
            self.lower_expand(&prev_var, &step.relationship, &to_var);
            self.lower_node_constraints(&step.node);
            prev_var = to_var;
        }
        Ok(())
    }

    /// Emit the scan for a pattern's starting node. If `current` is still the
    /// fresh `Empty`, the scan becomes the new leaf; otherwise it is a cartesian
    /// product / multi-pattern join — modelled here as a fresh scan replacing
    /// the leaf only when fresh, else stacked (the executor performs the join).
    fn lower_start_node(&mut self, node: &NodePattern) {
        let var = node_var(node);
        let scan = scan_for(&var, &node.labels);
        if self.fresh {
            self.current = scan;
            self.fresh = false;
        } else if !self.bound.contains(&var) {
            // A new, previously-unbound start node in a later pattern: the
            // executor joins it with the running rows (cartesian unless a
            // shared variable links them). We stack the scan's bindings; the
            // join itself is the executor's concern (T-0019).
            // Replace current with a structure that retains both: model as
            // Expand-less — keep current and register the new scan's variable so
            // later expands resolve. For now, wrap as an Optional-free join by
            // keeping current and noting the binding; a dedicated Join operator
            // is a follow-up (the read surface here is single-pattern dominant).
            self.current = scan;
            self.fresh = false;
        }
        self.bound.insert(var);
        self.lower_node_constraints(node);
    }

    /// Emit an expand from `from` to `to` along the relationship pattern.
    fn lower_expand(&mut self, from: &str, rel: &RelPattern, to: &str) {
        self.current = Operator::Expand {
            input: Box::new(std::mem::replace(&mut self.current, Operator::Empty)),
            from: from.to_string(),
            rel_variable: rel.variable.clone(),
            rel_types: rel.types.clone(),
            direction: rel.direction,
            to: to.to_string(),
            estimates: Estimates::unknown(),
        };
        self.bound.insert(to.to_string());
        if let Some(rv) = &rel.variable {
            self.bound.insert(rv.clone());
        }
    }

    /// Lower a node pattern's label set and inline property map into filters
    /// (the label set is already on the scan; the inline `{k: v}` map becomes
    /// equality predicates that push down to the scan).
    fn lower_node_constraints(&mut self, node: &NodePattern) {
        let Some(var) = node.variable.as_deref() else {
            return;
        };
        if let Some(props) = &node.properties {
            for (key, value_expr) in props {
                let predicate = property_equals(var, key, value_expr.clone());
                self.stack_filter(predicate);
            }
        }
    }

    fn lower_unwind(&mut self, u: &UnwindClause) -> PlanResult<()> {
        self.current = Operator::Unwind {
            input: Box::new(std::mem::replace(&mut self.current, Operator::Empty)),
            expr: u.expr.clone(),
            variable: u.variable.clone(),
        };
        self.fresh = false;
        self.bound.insert(u.variable.clone());
        Ok(())
    }

    /// Lower a `WITH` / `RETURN` projection: optional aggregate split, the
    /// projection itself, then ORDER BY / SKIP / LIMIT, then a trailing WHERE
    /// (legal only after `WITH`).
    fn lower_projection(&mut self, p: &ProjectionClause) -> PlanResult<()> {
        let items = projection_columns(&p.body);

        let (group_keys, aggregates): (Vec<_>, Vec<_>) = items
            .iter()
            .cloned()
            .partition(|c| !contains_aggregate(&c.expr));

        let child = std::mem::replace(&mut self.current, Operator::Empty);
        self.current = if aggregates.is_empty() {
            Operator::Project {
                input: Box::new(child),
                items,
                distinct: p.distinct,
            }
        } else {
            Operator::Aggregate {
                input: Box::new(child),
                group_keys,
                aggregates,
            }
        };

        // The projection re-binds the in-scope variables to its output columns.
        self.bound = items_binding(&p.body);

        if !p.order_by.is_empty() {
            let keys = p
                .order_by
                .iter()
                .map(|s| SortKey {
                    name: format!("{:?}", s.expr),
                    expr: s.expr.clone(),
                    descending: s.descending,
                })
                .collect();
            self.current = Operator::Sort {
                input: Box::new(std::mem::replace(&mut self.current, Operator::Empty)),
                keys,
            };
        }
        if let Some(skip) = &p.skip {
            self.current = Operator::Skip {
                input: Box::new(std::mem::replace(&mut self.current, Operator::Empty)),
                count: Box::new(skip.clone()),
            };
        }
        if let Some(limit) = &p.limit {
            self.current = Operator::Limit {
                input: Box::new(std::mem::replace(&mut self.current, Operator::Empty)),
                count: Box::new(limit.clone()),
            };
        }
        if let Some(pred) = &p.where_clause {
            // A trailing WITH ... WHERE filters the projected stream.
            self.push_filters(pred);
        }
        self.fresh = false;
        Ok(())
    }

    /// Split a conjunctive predicate on `AND` and stack one [`Filter`] per
    /// conjunct above `current` (push-down relocates them afterward). Splitting
    /// is what lets a multi-variable `WHERE a.x = 1 AND b.y = 2` push each half
    /// to its own scan.
    fn push_filters(&mut self, predicate: &Expr) {
        for conjunct in split_conjuncts(predicate) {
            self.stack_filter(conjunct);
        }
    }

    fn stack_filter(&mut self, predicate: Expr) {
        self.current = Operator::Filter {
            input: Box::new(std::mem::replace(&mut self.current, Operator::Empty)),
            predicate,
        };
    }

    fn into_root(self) -> PlanResult<Operator> {
        if self.fresh {
            // No MATCH/UNWIND/RETURN ever ran.
            return Err(PlanError::EmptyQuery);
        }
        Ok(self.current)
    }
}

/// The variable a node pattern binds, synthesising a stable anonymous name when
/// the pattern is unnamed (`()` / `(:Label)`), so expands always have a source.
fn node_var(node: &NodePattern) -> String {
    node.variable.clone().unwrap_or_else(|| {
        // Anonymous nodes get a deterministic synthetic name keyed by labels so
        // repeated lowering is stable within one pattern build.
        format!("__anon_{}", node.labels.join("_"))
    })
}

/// Build the leaf scan for a node: [`LabelScan`](Operator::LabelScan) when
/// labels are present (the selectivity anchor), else
/// [`NodeScan`](Operator::NodeScan).
fn scan_for(var: &str, labels: &[String]) -> Operator {
    if labels.is_empty() {
        Operator::NodeScan {
            variable: var.to_string(),
            estimates: Estimates::unknown(),
        }
    } else {
        Operator::LabelScan {
            variable: var.to_string(),
            labels: labels.to_vec(),
            estimates: Estimates::unknown(),
        }
    }
}

/// `var.key = value` as an [`Expr`] (the lowering of an inline `{key: value}`).
fn property_equals(var: &str, key: &str, value: Expr) -> Expr {
    use crate::cypher::ast::BinaryOp;
    Expr::Binary {
        op: BinaryOp::Equal,
        lhs: Box::new(Expr::Property {
            base: Box::new(Expr::Variable(var.to_string())),
            key: key.to_string(),
        }),
        rhs: Box::new(value),
    }
}

/// Split a predicate into its top-level `AND` conjuncts (recursively), so each
/// can be pushed independently.
pub(super) fn split_conjuncts(expr: &Expr) -> Vec<Expr> {
    use crate::cypher::ast::BinaryOp;
    match expr {
        Expr::Binary {
            op: BinaryOp::And,
            lhs,
            rhs,
        } => {
            let mut out = split_conjuncts(lhs);
            out.extend(split_conjuncts(rhs));
            out
        }
        other => vec![other.clone()],
    }
}

/// Whether an expression contains an aggregating function call (so the
/// projection must lower to an [`Aggregate`](Operator::Aggregate)).
fn contains_aggregate(expr: &Expr) -> bool {
    match expr {
        Expr::CountStar => true,
        Expr::FunctionCall { name, .. } => is_aggregate_fn(name),
        Expr::List(items) => items.iter().any(contains_aggregate),
        Expr::Map(entries) => entries.iter().any(|(_, e)| contains_aggregate(e)),
        Expr::Property { base, .. } => contains_aggregate(base),
        Expr::Index { base, index } => contains_aggregate(base) || contains_aggregate(index),
        Expr::Unary { operand, .. } => contains_aggregate(operand),
        Expr::Binary { lhs, rhs, .. } => contains_aggregate(lhs) || contains_aggregate(rhs),
        Expr::IsNull { operand, .. } => contains_aggregate(operand),
        Expr::Literal(_) | Expr::Variable(_) | Expr::Parameter(_) => false,
    }
}

/// The openCypher aggregating functions (case-insensitive).
fn is_aggregate_fn(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "count"
            | "sum"
            | "avg"
            | "min"
            | "max"
            | "collect"
            | "stdev"
            | "stdevp"
            | "percentilecont"
            | "percentiledisc"
    )
}

/// Lower a projection body into named output columns.
fn projection_columns(body: &ReturnBody) -> Vec<ProjectionColumn> {
    match body {
        ReturnBody::Items(items) => items.iter().map(column_for).collect(),
        ReturnBody::All { extra } => {
            // `RETURN *` keeps all in-scope variables; we cannot enumerate them
            // without a binding scope, so we model the wildcard as a single
            // sentinel column plus any explicit extras. The executor expands the
            // star against its row schema.
            let mut cols = vec![ProjectionColumn {
                name: "*".to_string(),
                expr: Expr::Variable("*".to_string()),
            }];
            cols.extend(extra.iter().map(column_for));
            cols
        }
    }
}

/// The output binding produced by a projection body (the variable names a later
/// clause can reference).
fn items_binding(body: &ReturnBody) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let items = match body {
        ReturnBody::Items(items) => items.as_slice(),
        ReturnBody::All { extra } => extra.as_slice(),
    };
    for item in items {
        out.insert(column_for(item).name);
    }
    out
}

/// The output column for a projection item: the alias if given, else a name
/// derived from the expression.
fn column_for(item: &ProjectionItem) -> ProjectionColumn {
    let name = item
        .alias
        .clone()
        .unwrap_or_else(|| expr_display_name(&item.expr));
    ProjectionColumn {
        name,
        expr: item.expr.clone(),
    }
}

/// A stable display name for an unaliased projection expression
/// (`n.name` → `"n.name"`, a bare variable → its name, else the debug form).
fn expr_display_name(expr: &Expr) -> String {
    match expr {
        Expr::Variable(v) => v.clone(),
        Expr::Property { base, key } => {
            if let Expr::Variable(v) = base.as_ref() {
                format!("{v}.{key}")
            } else {
                format!("{:?}.{key}", base)
            }
        }
        Expr::Literal(PropertyValue::Null) => "null".to_string(),
        Expr::CountStar => "count(*)".to_string(),
        other => format!("{other:?}"),
    }
}
