---
id: T-0035
title: License-clean synthetic graph dataset generator (1M nodes / 10M edges)
type: task
status: ready
priority: P2
assignee:
epic: EPIC-009
deps: [T-0006]
rubric_refs: [10]
estimate: S
created: T0+0:20
updated: T0+0:20
---

## Context

Benchmarks and integration tests need a representative graph with text properties.
Per the open-source guardrails, datasets must be license-clean — a generator is the
safest route (no third-party data committed). It must produce a power-law degree
distribution (with super-nodes) so the latency-envelope and out-of-envelope tests
are realistic. Depends only on the data model (T-0006). See `EPIC-009`,
`docs/process/open-source-guardrails.md`.

## Acceptance criteria
- [ ] Generator produces a graph of configurable size (default 1M nodes / 10M edges) with labels, text properties, and directed typed edges.
- [ ] Degree distribution is power-law (includes super-nodes) so it exercises the tail fan-out case (SPIKE-0004).
- [ ] Output is deterministic given a seed; written via the storage writers (or a portable format) so benches/tests can load it.
- [ ] No third-party data committed; the generator script + a small committed sample are the only artifacts (large graphs are gitignored / regenerated).
- [ ] tests added (generator unit tests; small-graph determinism); coverage not regressed
- [ ] docs updated with generation instructions + the license note (generated = no external license)
- [ ] `./format_code.sh` green

## Notes / log
Ready now: depends only on T-0006. Feeds T-0016 (headline bench), T-0020 (aggregate
bench), and integration tests across epics.
