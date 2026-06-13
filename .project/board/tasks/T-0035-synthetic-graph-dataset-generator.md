---
id: T-0035
title: License-clean synthetic graph dataset generator (1M nodes / 10M edges)
type: task
status: in_review
priority: P2
assignee: implementer-wf_156e2b80-bb6-11
epic: EPIC-009
deps: [T-0006]
rubric_refs: [10]
estimate: S
created: T0+0:20
updated: T0+3:25
---

## Context

Benchmarks and integration tests need a representative graph with text properties.
Per the open-source guardrails, datasets must be license-clean — a generator is the
safest route (no third-party data committed). It must produce a power-law degree
distribution (with super-nodes) so the latency-envelope and out-of-envelope tests
are realistic. Depends only on the data model (T-0006). See `EPIC-009`,
`docs/process/open-source-guardrails.md`.

## Acceptance criteria
- [x] Generator produces a graph of configurable size (default 1M nodes / 10M edges) with labels, text properties, and directed typed edges.
- [x] Degree distribution is power-law (includes super-nodes) so it exercises the tail fan-out case (SPIKE-0004).
- [x] Output is deterministic given a seed; written via the storage writers (or a portable format) so benches/tests can load it.
- [x] No third-party data committed; the generator script + a small committed sample are the only artifacts (large graphs are gitignored / regenerated).
- [x] tests added (generator unit tests; small-graph determinism); coverage not regressed
- [x] docs updated with generation instructions + the license note (generated = no external license)
- [x] `./format_code.sh` green

## Notes / log
Ready now: depends only on T-0006. Feeds T-0016 (headline bench), T-0020 (aggregate
bench), and integration tests across epics.

- T0+3:25 — implemented on branch `work/T-0035-synthetic-graph-dataset-generator`
  (worktree `.claude/worktrees/wf_156e2b80-bb6-11`). `src/dataset/` = vendored
  SplitMix64 PRNG → power-law generator (rank-Zipf, O(nodes) memory) → portable
  JSONL IO → `generate-dataset` CLI. Default 1M/10M; deterministic per seed;
  super-node tail (SPIKE-0004). Committed 6 KB sample pinned by an integration
  test; large graphs gitignored. No new deps (serde_json promoted dev→normal).
  Rebased onto main after T-0017 landed. 180 tests green, format_code.sh green.
  Status → in_review; PR.md filled; dispatching adversarial-reviewer + premortem.
