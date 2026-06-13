---
id: BUG-0019
title: Index facade Equals uses orderability equality, not the openCypher `=` operator — wrong rows for null/NaN
type: bug
status: ready
priority: P1
assignee:
epic: EPIC-005
deps: []
rubric_refs: [5, 4]
estimate: S
created: T+3:40
updated: T+3:40
---

## Context

Found during adversarial review of T-0022 (already landed on `main`, commit
`ab5fc7a`; the re-review branch `work/T-0022-pluggable-index-trait` is byte-identical).

`src/index/mod.rs` documents `IndexQuery::Equals(PropertyValue)` as
`WHERE n.prop = <value>` and the blanket `PropertyIndex` impl resolves it with
`self.lookup(&OrderedKey(v.clone()))`. `OrderedKey` equality is defined as
`cypher_order(a,b) == Equal` (orderability), **not** the openCypher `=` operator
(`PropertyValue::cypher_equal`, ternary). These two relations disagree exactly
where openCypher is subtle:

- **NaN:** `cypher_order(NaN, NaN) == Equal`, so `OrderedKey(NaN) == OrderedKey(NaN)`,
  but the `=` operator gives `cypher_equal(NaN, NaN) == Some(false)`. A
  `WHERE n.prop = NaN` predicate routed through the index returns the NaN-valued
  nodes; the correct answer is **no rows**.
- **null:** `OrderedKey(Null) == OrderedKey(Null)`, but `cypher_equal(null, null)
  == None` (unknown ⇒ no rows). A `WHERE n.prop = null` predicate routed through
  the index returns the null-keyed nodes; the correct answer is **no rows**
  (the spec way to match nulls is `IS NULL`, not `=`).

Reproduced empirically against the landed code (probe example):
```
NaN  equals probe -> [NodeId(1), NodeId(2)]   (correct: [])
null equals probe -> [NodeId(9)]              (correct: [])
int(1) equals probe vs float(1.0) -> [NodeId(5)]  (correct: [5] — this case is fine)
```

Impact: this is the contract T-0023 (B-tree) and T-0024 / EPIC-002 (planner) build
on. A planner that wires `WHERE n.prop = x` → `IndexQuery::Equals(x)` → `probe`
(exactly what the doc and ADR 0005 rubric-impact table instruct) emits wrong rows
for null/NaN equality — a silent Cat. 4 TCK regression, the precise risk ADR 0005
Alternative C cites. No live query path consumes it **today** (planner is a stub),
so it is latent, not a live incident — hence P1, fix before T-0024/EPIC-002 land.

## Acceptance criteria
- [ ] `IndexQuery::Equals` resolution matches the openCypher `=` operator: a `null`
      probe value and a `NaN` probe value both yield zero rows; `null`/`NaN` stored
      values are never returned by an `=` probe. (Decide the surface: either
      `Equals` is documented as identity-keyed lookup and the planner is forbidden
      from routing `= null`/`= NaN` through it, **or** the facade applies
      `cypher_equal` filtering after the ordered lookup. The interface doc must not
      claim `= <value>` semantics it does not provide.)
- [ ] Tests cover: `probe(Equals(null))` → `[]`; `probe(Equals(NaN))` → `[]`;
      stored `null`/`NaN` not returned; `1 = 1.0` still matches.
- [ ] Doc comment on `IndexQuery::Equals` and ADR 0005 corrected to state the exact
      equality semantics provided.
- [ ] `./format_code.sh` green; coverage not regressed.

## Notes / log
- T+3:40 filed by adversarial-reviewer during T-0022 re-review. Pairs with BUG-0020
  (range selectivity on equality-only index).
