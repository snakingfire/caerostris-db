---
id: BUG-0006
title: TCK side-effect assertions are unobservable without a QueryStatistics surface
type: bug
status: in_review
priority: P0
assignee: implementer-wf_84c0f0c7-752-19
epic: EPIC-002
deps: []
rubric_refs: [4, 10]
estimate: S
created: 2026-06-13T18:24:00Z
updated: 2026-06-13T19:06:00Z
---

## Context

Filed by `steering-query-cypher` during the launch ratification pass over
`docs/commanders-intent.md` and `docs/requirements/master-rubric.md`.

Cat. 4 (master-rubric.md L61-64) scores the project as the **raw openCypher TCK
pass-rate** and requires **100%**. A non-trivial class of TCK scenarios
(`CREATE`/`MERGE`/`SET`/`REMOVE`/`DELETE`, and many P2 transaction scenarios)
assert side effects, not result rows:

```gherkin
Then the side effects should be:
  | +nodes      | 1 |
  | +properties | 2 |
  | -properties | 1 |
  | +labels     | 1 |
```

These outcomes are **not observable from the query result set** — e.g.
`CREATE (n) DELETE n` returns no rows but must report `+nodes 1 / -nodes 1`.
This is a documented, contested gap in the TCK itself (openCypher/openCypher
Issue #221, "Unobservable behavior in TCK test"). Neo4j satisfies it via a
`QueryStatistics` object the harness reads; openCypher historically argued some
side-effect accounting (notably property +/- counts) is partly
implementation-defined.

**Why this is blocking:** `master-rubric.md`, `EPIC-002`, and `T-0002` (TCK
harness) describe an adapter that runs `When executing query <cypher>` and
asserts on the *result*. None require the engine to expose a **side-effect
accounting surface** (`+nodes`/`-nodes`, `+relationships`/`-relationships`,
`+labels`/`-labels`, `+properties`/`-properties`), nor require the adapter to read
and assert it. Without this surface, every side-effect scenario is
**structurally unpassable regardless of engine correctness** — so Cat. 4 = 100 is
unreachable. Per my non-negotiable ("escalate immediately if a proposal
structurally cannot reach 100%"), filed P0.

**Does NOT block launch.** Read-only P1 scenarios (the first, highest-weight
tranche) are unaffected. Must be resolved before P2 (writes+txns) work is `ready`.

## Acceptance criteria

- [x] Engine runtime exposes a `QueryStatistics`-equivalent side-effect counter
      covering every side-effect category the pinned TCK asserts:
      `+nodes`/`-nodes`, `+relationships`/`-relationships`, `+labels`/`-labels`,
      `+properties`/`-properties`. (`+indexes`/`-indexes`,
      `+constraints`/`-constraints` explicitly deferred — out of scope for the
      P1/P2 tranches this unblocks; one-line extension path recorded in
      Decision 0012.) → `src/query/stats.rs`.
- [~] T-0002's TCK adapter reads this surface and asserts the
      `Then the side effects should be:` step, counting such scenarios as real
      pass/fail (never auto-`pending`). The **contract** is defined here
      (`from_tck_side_effects` + `matches_side_effects`) and the end-to-end test
      exercises the adapter's exact assertion path; the harness itself lands in
      T-0002, whose acceptance criteria now mandate this read (BUG-0006 resolved
      the structural blocker; wiring is T-0002's job).
- [x] Property +/- counting semantics (the Issue #221 ambiguity) pinned to the
      official TCK's expected values for the pinned release (BUG-0007) and recorded
      in a decision doc → `.project/decisions/0012-tck-side-effect-counting-semantics.md`;
      the deferred `+indexes`/`+constraints` categories are documented there, not
      silently skipped.
- [x] One TCK side-effect scenario passes end-to-end as evidence →
      `tests/tck_side_effects.rs::create_then_delete_node_side_effects_pass`
      (`CREATE (n) DELETE n` → `+nodes 1 / -nodes 1`).
- [x] EPIC-002 and T-0002 acceptance criteria updated to name this surface.
- [x] `./format_code.sh` green.

## Notes / log
- T0 `steering-query-cypher`: filed during ratification. Decision recorded at
  `.project/decisions/0007-tck-side-effect-observability.md`. Fix owner: a
  `planner-decomposer` should fold the side-effect surface into the EPIC-002
  executor/runtime story and the T-0002 adapter interface. Cross-check with
  `steering-distributed-acid` for transaction-scenario side effects.
- T0+0:42 `implementer-wf_84c0f0c7-752-19`: claimed; implemented the
  `caerostris_db::query::QueryStatistics` surface TDD-first
  (`src/query/stats.rs`, `tests/tck_side_effects.rs`). Counting semantics pinned
  in Decision 0012. EPIC-002 + T-0002 acceptance criteria amended to mandate the
  adapter read this surface. PR worktree: `.worktrees/BUG-0006`,
  branch `work/BUG-0006-...`. Status → in_review.
