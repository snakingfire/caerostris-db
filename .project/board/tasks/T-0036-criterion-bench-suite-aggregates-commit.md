---
id: T-0036
title: Criterion bench suite for aggregates + commit throughput, with baselines
type: task
status: backlog
priority: P2
assignee:
epic: EPIC-009
deps: [T-0020, T-0010, T-0035]
rubric_refs: [10, 6]
estimate: S
created: T0+0:20
updated: T0+0:20
---

## Context

Cat. 10 = 100 requires criterion benches for the headline query (T-0016),
aggregates, and commit throughput, with baselines committed and regression
detection. This task covers the aggregate + commit-throughput benches (the headline
query bench is T-0016) and wires baseline storage + a regression threshold. See
`EPIC-009`.

## Acceptance criteria
- [ ] Criterion benches for: aggregates (count/sum/distinct) and commit throughput, run on the T-0035 dataset.
- [ ] Baselines committed under `benches/baselines/`; a regression threshold is configured and CI warns (or fails) on regression beyond it.
- [ ] Aggregate bench demonstrates the layout-accelerated path (T-0020) beats a naïve full-scan baseline.
- [ ] Bench results are emitted in a format the grader can cite.
- [ ] tests/benches added; coverage not regressed
- [ ] docs updated with the bench methodology
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: depends on aggregates (T-0020), commit (T-0010), and the
dataset (T-0035). Complements T-0016 (headline cold-start bench).
