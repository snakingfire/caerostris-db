---
id: SPIKE-0003
title: Specify on-object storage format layout
type: spike
status: in_progress
priority: P0
assignee: researcher
epic: EPIC-001
deps: [SPIKE-0001]
rubric_refs: [2]
estimate: M
created: T0
updated: 2026-06-13T21:00:00Z
---

## Context

This spike produces the **storage format specification** — the detailed on-object layout that all writers and readers must conform to. It is `backlog` (not `ready`) because it depends on SPIKE-0001: the format must demonstrably serve the latency-envelope byte budget (B_max) derived there, and the column/adjacency partitioning choices are constrained by the range-read access patterns the cost model requires.

The format must satisfy (per R4):
- **All durable state on S3-compatible object storage.** No POSIX dependency for durability.
- **Few, large, parallelizable range GETs.** The layout must allow the planner to request a contiguous byte range that covers exactly the data it needs for a query, without fetching irrelevant data. The partitioning granularity (e.g. node-property column per partition, adjacency list chunks) must be chosen so a typical in-envelope query reads ≤ B_max total across ≤ K sequential phases.
- **Columnar / adjacency layout**: nodes stored columnar (one column per property type, sorted by node ID for range access); edges stored as compressed adjacency lists (sorted by source node ID); cross-references allow efficient hop expansion.
- **Versioned + GC-able**: each committed version has a manifest object listing all data objects it references; old versions are GC-ed by deleting manifest + unreferenced objects. Object names include a version or content hash so writes are always new objects (no in-place mutation on S3).
- **Self-describing**: the manifest includes schema metadata, format version, and statistics (node count, edge count, property cardinalities) useful for query planning.
- **Forward-thinking about schema**: the format should tolerate adding new property columns without rewriting existing data objects.

The format spec is a companion to the protocol ADR from SPIKE-0002 (which covers the commit mechanics); together they fully specify what is on the object store and how it gets there.

Steering sign-off: **steering-storage** must approve the format spec before T-0001 (crate skeleton + storage abstraction) extends beyond the stub phase and before any format-writing implementation task becomes `ready`.

## Acceptance criteria

- [ ] Format spec committed to `docs/adr/` (e.g. `docs/adr/0003-storage-format.md`): describes all object types (manifest, node-property column objects, adjacency-list objects, index objects), their naming conventions, internal binary layout (field order, encoding, alignment), and the range-read access pattern for a representative query.
- [ ] Byte-budget analysis in the spec: shows that for the in-envelope selectivity from SPIKE-0001, a 6-hop query reads ≤ B_max total bytes across ≤ K phases given the chosen partition sizes.
- [ ] Versioning and GC strategy specified: manifest structure, how a reader pins a version, how GC identifies and deletes unreferenced objects safely.
- [ ] Schema evolution strategy documented: how new property columns are added without rewriting existing data objects.
- [ ] Format spec cross-referenced from EPIC-001 and from SPIKE-0001 output.
- [ ] Steering-storage sign-off recorded in `.project/decisions/`.
- [ ] No Rust implementation required — specification only.

## Notes / log

Status is `backlog` pending SPIKE-0001 ratification. Once SPIKE-0001 is done, this task should be flipped to `ready` by the planner-decomposer. The implementation counterpart is T-0001 (crate skeleton + object-store abstraction).

- **T+0:06 (steering-storage):** This spec must discharge the storage-domain
  falsification constraints in `SPIKE-0008` (filed during the intent/rubric
  ratification pass) — specifically **F1** (early-abort partial adjacency reads /
  adjacency chunking so the binding 50 Mbps case is feasible) and the storage side
  of **F2** (name + verify the conditional-PUT primitive used for the manifest
  swap) and **F3** (safe-GC-vs-reader policy: retention grace window / TTL'd pins).
  `steering-storage` will not ratify this spec until F1/F2/F3 are explicitly
  addressed. See `.project/decisions/0001-storage-domain-ratification-findings.md`.
- **T+~01:28 steering-formal-methods note (decision 0015, ADR 0001 finding F2):**
  the early-abort partial adjacency read (SPIKE-0008 F1) must be specified as a
  **hard per-GET byte/row cap** — the reader truncates an adjacency range-GET once the
  running LIMIT/byte budget is consumed. This is the mechanism that bounds realized bytes
  even when a frontier node is a super-hub (out-degree ≫ p99), making the super-hub case a
  *detection*-only concern (handled by T-0015/SPIKE-0004's max-degree estimator) rather
  than a realized-latency bust. Restated here so the format spec wires early-abort as a
  budget-driven hard cap, not merely an optional optimization.
- **T+~04:05 SPIKE-0004 ratified-spec input (`docs/specs/SPIKE-0004-manifest-statistics-contract.md`,
  sign-off `.project/decisions/0030-...`):** the manifest statistics contract is now pinned and
  this format spec owns two of its storage-layer decisions, per SPIKE-0004 Part 2 / R1:
  (a) the **inline-vs-referenced cut** for the statistics block — OOE-critical scalars
  (`node_count`, `total_node_count`, `edge_count`, `p99_deg`, `max_deg`) inline in the manifest,
  bulky per-property MCV/histogram detail optionally a referenced content-addressed
  `db/stats/<hash>.stats` blob GC-ed via the same manifest-reference-set rule (ADR 0002 §6); the
  binding invariant to preserve is "super-hub / non-selective rejection needs no data-plane GET
  beyond the manifest"; and (b) the early-abort per-GET byte/row cap above (F2's realized-latency
  protection). Fold both into the format spec before `steering-storage` ratifies.
  Value-digest privacy (per SPIKE-0004 Part 1.2): MCV/histogram entries store fixed-width
  collision-resistant digests (BLAKE3-truncated) + order-preserving truncated keys, **never raw
  property values** — keeps committed fixtures free of user data by construction (guardrails §3).
