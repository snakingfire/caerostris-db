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
updated: T0+3:37
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
- T0+3:30 — adversarial-reviewer: **approve** (verdict in PR.md). Verified locally
  in-worktree: build clean, `cargo test` all green (153 lib + 3 dataset_sample
  integration), clippy `--workspace --all-targets --all-features -D warnings`
  clean, `cargo fmt --check` clean. Attacks (cross-platform `powf`, empty/1-node/
  divide-by-zero, float round-trip, range/self-loop, guardrails) all survived;
  the one residual platform-determinism caveat applies only to non-unit Zipf
  exponents (no committed fixture uses one) — non-blocking. Still needs
  premortem-analyst sign-off before the integrator can land.
- T0+3:37 — premortem-analyst: **approve** (verdict in PR.md; premortem box ticked).
  Worked backwards across all six lenses. The four P0 lenses (silent ACID/data
  corruption, SLA-theorem regression, split-brain, irreversible state) have no
  surface: this is offline generator tooling that never touches the durable store,
  commit protocol, leases, GC, reader pins, or the in-envelope query path. The one
  genuine corruption vector — float-text round-trip drift — is mitigated by
  6-decimal quantisation and proven byte-exact (`larger_graph_round_trips...`,
  byte-exact pinned-sample test). Two residual OPERATIONAL notes accepted: (1)
  cross-platform `powf` only for non-default `--zipf` (no committed/default
  artifact uses it; `powf(1.0)` is IEEE-exact); (2) large default file goes to the
  gitignored `data/`. Re-verified gates in-worktree: `./format_code.sh` green
  (exit 0), `cargo nextest run` 180/180 pass. No latent bug found — no BUG filed.
  Both review-gate sign-offs now `approve`; ready for the integrator.
