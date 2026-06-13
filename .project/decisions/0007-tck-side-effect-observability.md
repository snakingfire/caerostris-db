# Decision 0007 — TCK side-effect observability requires a QueryStatistics surface

- **Date / marker:** T0 (2026-06-13T18:24:00Z)
- **Owner:** steering-query-cypher
- **Status:** recorded; tracked by BUG-0006 (P0)
- **Rubric:** Cat. 4 (openCypher/TCK), Cat. 10

## Context

Launch ratification pass over commanders-intent.md and master-rubric.md. Cat. 4
is a GATE scored as the raw openCypher TCK pass-rate with a 100% bar.

## Finding

A class of TCK scenarios assert query **side effects** (`+nodes`, `-nodes`,
`+relationships`, `-relationships`, `+labels`, `-labels`, `+properties`,
`-properties`) via `Then the side effects should be:` steps. These are not
observable from the query result set (e.g. `CREATE (n) DELETE n`). Confirmed
against openCypher/openCypher Issue #221 ("Unobservable behavior in TCK test"),
which documents that some side-effect accounting is reported via a
`QueryStatistics`-style surface and that property +/- counting was historically
contested as implementation-defined.

The rubric, EPIC-002, and T-0002 only describe asserting on results. Without an
engine-exposed side-effect counter and a TCK adapter that reads it, every
side-effect scenario is structurally unpassable → Cat. 4 = 100 unreachable.

## Decision

- Treat this as a structural blocker to 100% TCK and file it P0 (BUG-0006),
  scoped to P2 (writes+txns), not blocking the P1 read tranche or launch.
- The engine runtime MUST expose a `QueryStatistics`-equivalent side-effect
  counter; the T-0002 adapter MUST read it and assert the side-effects step as
  real pass/fail (never auto-`pending`).
- Property +/- counting semantics pinned to the official TCK's expected values
  for the pinned release (see Decision 0008); genuinely unobservable/optional
  scenarios documented, not silently skipped.

## Alternatives considered

- **Auto-`pending` all side-effect scenarios.** Rejected: that is a curated
  subset and falsifies "100% means all of it" (commanders-intent.md L31).
- **Infer side effects by re-querying state after the write.** Rejected as a
  general mechanism: deletes/no-op writes and property count deltas are not
  reconstructable from post-state alone; fragile and scenario-specific.

## Consequences

EPIC-002 executor/runtime story and the T-0002 adapter interface gain a
side-effect accounting contract. Cross-checked with steering-distributed-acid for
transaction-scenario side effects.
