---
id: T-0009
title: Implement manifest object, statistics block, and latest-version resolution
type: task
status: done
priority: P1
assignee: implementer-wf_156e2b80
epic: EPIC-001
deps: [SPIKE-0002, SPIKE-0003, SPIKE-0004]
rubric_refs: [1, 2, 3]
estimate: M
created: T0+0:20
updated: T0+3:58
---

## Context

The manifest is the root object that lists every data object a committed version
references, carries format version + schema metadata, and (per `SPIKE-0004` /
decision 0009) carries the **maintained graph statistics** the planner reads
snapshot-consistently for out-of-envelope detection. Latest-version resolution
must use the primitive SPIKE-0002 names (CAS pointer or uniquely-named immutable
manifest + list/max). Design-gated on SPIKE-0002 (commit primitive), SPIKE-0003
(object naming/layout), and SPIKE-0004 (statistics contract). See `EPIC-001`,
`EPIC-004`.

## Acceptance criteria
- [ ] Manifest schema implemented per SPIKE-0003 + SPIKE-0004 (`docs/specs/SPIKE-0004-manifest-statistics-contract.md`): object references, format version, schema metadata, and the statistics block (per-label `node_count` + `total_node_count`; per-property selectivity — NDV/null_frac/MCV/histogram; per-rel-type `edge_count`/`p99_deg` tail term **and the mandatory `max_deg` super-hub safety term** per decision 0015 / ADR 0001 F2; block metadata: `stats_version`/`as_of_version`/`freshness`/`estimator_params`). OOE-critical scalars (`node_count`, `total_node_count`, `edge_count`, `p99_deg`, `max_deg`) inline; bulky per-property MCV/histogram detail may be a referenced content-addressed `db/stats/<hash>.stats` blob (final inline-vs-referenced cut is steering-storage's per SPIKE-0004 Part 2.1 / SPIKE-0003).
- [ ] Latest-version resolution implemented exactly as SPIKE-0002's ADR specifies (named primitive + consistency assumption); documented in code.
- [ ] A reader resolving manifest M can enumerate and read every object M references (durability-barrier invariant from SPIKE-0005 Constraint 3) — integration-tested on the mock.
- [ ] Statistics are readable from the pinned manifest with no extra round-trip beyond resolving the manifest itself.
- [ ] tests added (unit + integration on the S3 mock); coverage not regressed
- [ ] docs / ADR updated if behaviour or architecture changed
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: blocked on SPIKE-0002, SPIKE-0003, SPIKE-0004. The statistics
block is the bridge to EPIC-003's planner detection (SPIKE-0004 / decision 0009).

- **T0+3:58 (implementer-wf_156e2b80):** implemented in `src/storage/manifest/`
  (`mod.rs` + `stats.rs` + `resolve.rs`) on branch
  `work/T-0009-manifest-and-version-resolution`, rebased on latest `main`
  (`ec47614`). 34 unit + 5 integration + 2 doctests green; `format_code.sh`
  green; no new deps. `status → in_review`; PR.md in the worktree root.
  Inline OOE-critical scalars + referenced `db/stats` selectivity blobs
  (condition C4), value-digest privacy, LIST+max resolution (ADR 0002 §2.3),
  durability-barrier enumeration (SPIKE-0005 C3). The "integration on the real
  S3 mock" half (no S3 `ObjectStore` adapter exists yet) is split out to
  **T-0048** so this could land now without a new S3-client dependency.
