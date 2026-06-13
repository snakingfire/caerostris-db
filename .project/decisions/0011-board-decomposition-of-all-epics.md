# Decision 0011 — Board decomposition of all 10 epics (T-0003)

- **Date / marker:** T0+0:20 (2026-06-13T18:44Z)
- **Owner:** `planner-decomposer` (executing T-0003)
- **Type:** Planning / board decomposition (not design-level; no steering gate)
- **Rubric:** advances Cat. 12 (process health); structures work toward all GATEs
- **Status:** accepted

## What was done

Decomposed every seeded epic (EPIC-001…EPIC-010) into concrete, independently
claimable stories/tasks with accurate `epic`, `deps`, `rubric_refs`, `priority`,
`estimate`, and ≥3 testable acceptance criteria each. Filed T-0006…T-0040,
STORY-001, and SPIKE-0009. After decomposition every epic has ≥3 children; the
ready queue holds 17 items (≥10 required for fan-out).

## Design-before-code enforcement (the load-bearing rule)

No design SPIKE is `done` yet. The `.project/decisions/` files (0001, 0004, 0005,
0007–0010) are steering **ratification-pass findings on the intent/rubric**, not
sign-offs of the spikes' design artifacts. Therefore every implementation task that
touches a design-gated path was filed `status: backlog` with the gating spike in
`deps`:

- **Storage-format path** (T-0007 columnar nodes, T-0008 adjacency edges, T-0009
  manifest, T-0023 B-tree-on-S3, T-0012 GC) → dep **SPIKE-0003** (which itself
  deps SPIKE-0001). Flip to `ready` only when SPIKE-0003 is steering-ratified
  (`steering-storage`, after discharging SPIKE-0008 F1/F2/F3).
- **Commit / ACID path** (T-0010 commit, T-0011 snapshot reads, T-0026 lease,
  T-0038 Apalache sync) → dep **SPIKE-0002** + **SPIKE-0005**. Flip only after
  steering-distributed-acid + steering-formal-methods sign-off and SPIKE-0005's
  three constraints (CAS primitive, fencing-into-predicate, durability barrier)
  are resolved in the ADR/model.
- **Latency path** (T-0014 sim, T-0015 out-of-envelope detection, T-0016
  cold-start bench) → dep **SPIKE-0001** + **SPIKE-0004/0006/0007** as applicable.
  Flip only after steering-perf-sla + steering-formal-methods sign-off.

## Board-honesty corrections made

The protocol (and T-0003 AC) forbids `status: ready` on any task with a non-`done`
dep. Two **inherited** scaffold items violated this and were corrected to
`backlog`:

- **T-0001** (crate skeleton) had dep `T-0000` (env provisioning) but was `ready`.
  T-0000 is not done → T-0001 set to `backlog`. T-0000 remains the single
  foundational `ready` entry point (no deps) that unblocks the storage/test
  substrate.
- **SPIKE-0004** had dep `SPIKE-0001` but was `ready` → set to `backlog`.

After this, the invariant "no `ready` task has a non-`done` dep" holds uniformly.

## Architectural assumptions recorded during decomposition

1. **Logical data model is format-independent** (T-0006): `Node`/`Edge`/
   `PropertyValue` land before SPIKE-0003 so parser, planner, indices, dataset
   generation, and Python bindings can proceed in parallel without waiting on the
   byte layout. The on-object format serialises these types.
2. **openCypher front-end is storage-independent** (T-0017 lexer/parser, T-0018
   logical planner/IR): these were initially filed `ready` but are gated only on
   T-0001 (crate skeleton), so they are `backlog` until T-0001 lands — then they
   parallelise immediately. The executor (T-0019) is the join point that needs the
   storage readers.
3. **Manifest carries maintained statistics** (T-0009) per decision 0009 /
   SPIKE-0004 — the planner reads selectivity + tail fan-out snapshot-consistently
   for out-of-envelope detection; no extra round-trip.
4. **Cache is a wrapper, not a layer in the engine** (T-0033/T-0040): disabling it
   is a single flag; the cold-SLA-with-cache-off guard (T-0034) protects the
   non-negotiable invariant.
5. **Datasets are generated, not vendored** (T-0035): a deterministic power-law
   generator (with super-nodes) keeps the repo license-clean and exercises the
   tail fan-out case the envelope detection must catch.
6. **STORY-001 (TCK Phase-3) is an `L` umbrella** — split into per-failing-bucket
   `S`/`M` child tasks against the live T-0002 report as the pass-rate climbs;
   not directly assignable as-is.

## Canonical paths

Per BUG-0003, ADRs go to `docs/adr/` (singular) and formal artifacts to `formal/`
(`formal/commit-protocol/`, `formal/latency-model/`, `formal/latency-sim/`). New
tasks reference these canonical paths, not the `docs/adrs/` / `docs/formal/` typos
in the seeded spikes.

## Follow-ups for the pace-marshal / next planner pass

- Flip storage/commit/latency implementation tasks to `ready` the moment their
  gating spike reaches `done` (SPIKE-0001 → SPIKE-0003 → T-0007/0008/0009 chain;
  SPIKE-0002/0005 → T-0010/0011 chain).
- Split STORY-001 into per-bucket child tasks once T-0002 reports a real
  pass/pending breakdown.
- As the grader files gap-closing tasks, decompose them under the relevant epic.
