---
id: BUG-0019
title: Index facade Equals uses orderability equality, not the openCypher `=` operator — wrong rows for null/NaN
type: bug
status: in_review
priority: P1
assignee: implementer-wf_fe688db0-093-31
epic: EPIC-005
deps: []
rubric_refs: [5, 4]
estimate: S
created: T+3:40
updated: T+4:09
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
- T+3:58 implemented on `work/BUG-0019-index-equals-cypher-equal-semantics` (based on
  latest `main`). Surface chosen: keep the documented `= <value>` semantics; guard
  `Equals(v)` in both `probe` and `selectivity` to short-circuit to no rows when
  `cypher_equal(v, v) != Some(true)` (the sole orderability-vs-`=` divergence: `null`,
  `NaN`, indeterminate containers). Clean probes need no post-filtering (proof in the
  helper doc + ADR 0005). 7 new TDD tests (4 RED→GREEN, 3 regression guards); full suite
  285 passed; clippy + `./format_code.sh` green. ADR 0005 + `IndexQuery::Equals` doc
  corrected. Status → in_review; review gate (adversarial-reviewer + premortem-analyst)
  pending.
- T+4:02 adversarial-reviewer: **approve**, no blocking findings. Central correctness
  claim ("clean probe needs no post-filtering") survived a 299-value brute force in
  both directions (zero false positives, zero false negatives) + precision-loss
  (`i64 as f64`) and self-equality audits. fmt/clippy/format_code.sh green; full
  `cargo test` 0 failures (lib 242, index 40). Verdict block + reviewer checkbox in
  PR.md. Non-blocking: confirm range-bound null/NaN is covered (BUG-0020 sibling)
  before the planner wires comparison predicates. Pre-mortem sign-off still pending.
- T+4:08 premortem-analyst: **approve**. Re-ran the load-bearing claim with an
  *independent* brute force (2500 ordered pairs over a 50-value corpus incl. NaN,
  ±0.0, ±Inf, i64::MAX, `2^53+1` precision-loss, and dirty/clean nested lists):
  for clean probes orderability-eq ⟺ `=`-eq (both directions), for dirty probes no
  stored value is ever `=`-equal — zero false positives, zero false negatives.
  `./format_code.sh` exit 0; full `cargo test` exit 0 (lib 242 + all integration
  suites, 0 failures); no dep/Cargo.lock change, no `unsafe`, no new unwrap/panic.
  Six pre-mortem lenses cleared (corruption mitigated; no SLA/byte/phase impact —
  O(1) guard on the hot path, no extra reads, not cache-dependent; no concurrency/
  manifest/GC surface; reversible). Non-blocking: the fix relies on cypher_equal /
  cypher_order staying mutually consistent — a future divergence beyond null/NaN
  would not be caught by a compile-time link (latent maintenance risk, not introduced
  here). Both review-gate sign-offs now green; ready for the integrator.
