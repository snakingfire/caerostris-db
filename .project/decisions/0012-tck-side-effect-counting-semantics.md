# Decision 0012 — TCK side-effect counting semantics and the `QueryStatistics` surface

- **Date / marker:** T0+0:42 (2026-06-13T~19:06:00Z)
- **Owner:** implementer-wf_84c0f0c7-752-19 (executing BUG-0006)
- **Status:** recorded; implements Decision 0007 (BUG-0006)
- **Rubric:** Cat. 4 (openCypher/TCK), Cat. 10 (tests/coverage)
- **Supersedes/extends:** Decision 0007 (side-effect observability — the *why*);
  this decision fixes the *what* (counting semantics) and the *how* (the engine
  surface + adapter contract).

## Context

Decision 0007 / BUG-0006 established that the engine must expose a
`QueryStatistics`-equivalent side-effect counter and that the T-0002 TCK adapter
must read it to assert `Then the side effects should be:` steps as real
pass/fail. BUG-0006 also requires the property `+`/`-` counting ambiguity
(openCypher/openCypher Issue #221, "Unobservable behavior in TCK test") to be
pinned to the official TCK's expected values for the pinned release (Decision
0008 / BUG-0007) and recorded here.

## Decision

### 1. The categories

`QueryStatistics` (`src/query/stats.rs`) carries the eight side-effect categories
the openCypher TCK emits, each a non-negative **occurrence count** (never a net
delta):

| TCK key           | accessor                  |
|-------------------|---------------------------|
| `+nodes`          | `nodes_created`           |
| `-nodes`          | `nodes_deleted`           |
| `+relationships`  | `relationships_created`   |
| `-relationships`  | `relationships_deleted`   |
| `+labels`         | `labels_added`            |
| `-labels`         | `labels_removed`          |
| `+properties`     | `properties_set`          |
| `-properties`     | `properties_removed`      |

`+indexes`/`-indexes` and `+constraints`/`-constraints` are **not** modelled
yet. Rationale: indexes/constraints are DDL-level side effects, the pinned TCK
release (Decision 0008 / BUG-0007 will fix the exact tag) asserts them only in
schema-command features that are out of scope for the P1 (read) and P2
(write+txn) tranches this surface unblocks. Adding a category to the engine is a
one-line change to the `CATEGORIES` table plus a counter field; a follow-up task
extends the surface when schema features come online. This is recorded so the
omission is explicit, not silent (BUG-0006 acceptance bullet 1).

### 2. Counting semantics (Issue #221 — pinned)

- A statement that creates **and** deletes the same entity reports both
  `+nodes 1` and `-nodes 1` (not `nodes 0`). Occurrence counts, not net.
- `+properties` counts each property *write* (create or overwrite-with-a-new
  value). `-properties` counts each property *removal*, where setting a property
  to `null` is the openCypher removal idiom.
- **Setting a property to the value it already holds is a no-op and is not
  counted** — this is the contested Issue #221 case. We pin to the TCK's
  expected values for the pinned release: the TCK's own expected side-effect
  tables for `SET`-to-same-value scenarios omit the `+properties` row (i.e.
  expect zero), so a no-op write must not increment `+properties`.
- A category **absent** from a TCK side-effect table is asserted to be `0`. The
  parser therefore returns a fully specified `QueryStatistics` (missing rows =
  zero) and equality compares every category. This makes the assertion total: an
  engine that produces an *extra*, unexpected side effect fails the scenario.

### 3. The adapter contract

- The engine runtime populates a `QueryStatistics` while applying a statement
  (recorders: `record_nodes_created`, `record_properties_removed`, …).
- The T-0002 TCK adapter:
  1. parses the Gherkin side-effect table with
     `QueryStatistics::from_tck_side_effects(table)` → `expected`;
  2. reads the engine's reported `QueryStatistics` → `actual`;
  3. asserts `actual.matches_side_effects(&expected)` (≡ `actual == expected`) as
     a real **pass/fail** — never auto-`pending`.
- On mismatch the adapter renders both via `to_tck_side_effects()` /
  `Display` for a readable diff.
- Genuinely unobservable/optional scenarios that the *TCK itself* marks as such
  are documented where they are skipped, never silently dropped (consistent with
  Decision 0008: nothing leaves the `total` denominator).

## Alternatives considered

- **Model `+indexes`/`+constraints` now.** Deferred, not rejected: no schema
  features exist to exercise them and the pinned release does not assert them in
  the read/write tranches this unblocks. Recorded as an explicit, reversible gap
  with a one-line extension path.
- **Net deltas instead of occurrence counts.** Rejected: the TCK expects
  occurrence counts (`CREATE (n) DELETE n` → `+nodes 1 / -nodes 1`), and net
  deltas are structurally unable to express that.
- **Count `SET`-to-same-value as `+properties 1`.** Rejected: contradicts the
  pinned TCK's expected tables for those scenarios (Issue #221).

## Consequences

- New engine surface `caerostris_db::query::QueryStatistics`
  (`src/query/stats.rs`), unit-tested + an end-to-end side-effect scenario test
  (`tests/tck_side_effects.rs`).
- EPIC-002 (executor/runtime) and T-0002 (TCK adapter) acceptance criteria
  amended to name this surface and the assertion contract.
- The exact pinned TCK release tag and the `+indexes`/`+constraints` decision are
  finalised under BUG-0007 (Decision 0008); when that pin lands, re-check whether
  any in-scope scenario asserts a category not yet modelled here and, if so, file
  the one-line extension task.
