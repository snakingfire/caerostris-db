---
id: EPIC-005
title: Pluggable secondary indices (B-tree on text properties first)
type: epic
status: backlog
priority: P1
assignee:
epic:
deps: []
rubric_refs: [5]
estimate: L
created: T0
updated: T0
---

## Context

Secondary indices (Cat. 5, weight 7) are the mechanism that makes the latency selectivity-envelope theorem workable in practice: a B-tree index on a node text property allows the planner to anchor an "unanchored" 6-hop MATCH to a tiny seed set, keeping bytes-read ≤ B_max. Without indices the envelope is too narrow to be useful; with them the conditional theorem becomes practically valuable.

This epic delivers: (1) a **pluggable index trait/interface** in Rust that any index type implements, so B-tree, range, full-text, spatial, and composite indices can be added without rewriting core planner or storage logic; (2) a **B-tree index on text node properties** as the first concrete implementation, stored on object storage (consistent with EPIC-001 format), updated transactionally with the main commit; (3) **planner integration** — the query planner consults available indices by selectivity and uses the B-tree when a WHERE clause filters on an indexed text property; and (4) **a second index type stubbed** against the same trait to demonstrate extensibility.

The index must be stored and committed atomically alongside the main data (one commit = data + index update, served by the EPIC-004 commit protocol). The B-tree on object storage must handle cold reads efficiently — ideally one or two range GETs to resolve a leaf lookup, so index access does not blow the latency budget.

Relevant requirements: R5 (pluggable indices, B-tree first), R7 (anchoring the latency envelope), R4 (storage on object store).

## Acceptance criteria

- [ ] Index trait/interface defined in Rust: covers insert, delete, point-lookup, and range-scan; a blanket planner API consults the trait without knowing the concrete type.
- [ ] B-tree index on text node properties implemented: lookup by equality and prefix; stored on object storage in a format compatible with EPIC-001's layout and committed atomically.
- [ ] Planner uses the B-tree index for WHERE clauses on indexed text properties: a query plan for `MATCH (n) WHERE n.name = 'X' ...` shows index lookup rather than full scan.
- [ ] Selectivity-aware planning: planner chooses index when estimated selectivity makes it cheaper; falls back to scan otherwise.
- [ ] A second index type (e.g. a range index or a stub full-text index) implemented against the same trait, confirming the interface is not leaking B-tree specifics.
- [ ] Index updates are transactionally consistent with data: a crashed commit does not leave index and data out of sync.
- [ ] Tests: unit tests for B-tree operations; integration test showing index-assisted query against the mock object store returning correct results with measurably fewer bytes read than a full scan.
- [ ] `./format_code.sh` green; no clippy warnings.

## Notes / log

Depends on EPIC-001 storage abstractions being available (the index lives on object storage) and on the planner architecture from EPIC-002. SPIKE-0003 (storage format spec) should be ratified first so index objects fit naturally into the layout.

**SPIKE-0004 (manifest statistics contract) feeds, and is fed by, the B-tree index.** Per-(label,property) `ndv`/`null_frac`/MCV/`histogram` statistics specified in `docs/specs/SPIKE-0004-manifest-statistics-contract.md` are the selectivity inputs the planner uses to choose the B-tree by selectivity (acceptance criterion 4 above). The histogram/MCV summaries support equality, range, and prefix selectivity for the B-tree; composite/correlated-predicate stats are noted as a future extension carried by this index trait (spec R3). Sign-off request: `.project/decisions/0030-spike-0004-statistics-contract-signoff-request.md`.
