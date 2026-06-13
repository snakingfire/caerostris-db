# PR: T-0035 — License-clean synthetic graph dataset generator (1M nodes / 10M edges)

## Board item

[.project/board/tasks/T-0035-synthetic-graph-dataset-generator.md](.project/board/tasks/T-0035-synthetic-graph-dataset-generator.md)

## Rubric refs

Cat. 10 (Tests, coverage & benchmarks) — provides the representative, scalable,
license-clean graph that the headline bench (T-0016), aggregate bench (T-0020),
and integration tests need. Also de-risks Cat. 3 (latency envelope): the
power-law super-nodes are the tail fan-out case (SPIKE-0004) those tests anchor on.

## Acceptance criteria (from board item)

- [x] Generator produces a graph of configurable size (default 1M nodes / 10M edges) with labels, text properties, and directed typed edges.
- [x] Degree distribution is power-law (includes super-nodes) so it exercises the tail fan-out case (SPIKE-0004).
- [x] Output is deterministic given a seed; written via the storage writers (or a portable format) so benches/tests can load it.
- [x] No third-party data committed; the generator script + a small committed sample are the only artifacts (large graphs are gitignored / regenerated).
- [x] tests added (generator unit tests; small-graph determinism); coverage not regressed
- [x] docs updated with generation instructions + the license note (generated = no external license)
- [x] `./format_code.sh` green

## Summary of change

Adds a self-contained synthetic property-graph generator under `src/dataset/`,
exposed as the `caerostris-db generate-dataset` CLI subcommand. It streams a
directed, typed, labelled graph (nodes carry `name`/`bio`/`country` text
properties + an integer `age`; edges are typed with `weight`/`rank`) at a
configurable size — default the headline **1M nodes / 10M edges**. The in-degree
distribution is **power-law with super-nodes** via rank-Zipf inverse-CDF target
sampling, which costs `O(node_count)` memory (the edge stream is constant-memory),
so 1M/10M generates in seconds with a few MB of RAM and exercises the tail
fan-out case SPIKE-0004 calls out.

Determinism is guaranteed by a **vendored, dependency-free SplitMix64 PRNG**
(public-domain reference algorithm): identical seed + size ⇒ byte-identical
output on every platform. We deliberately did **not** add the `rand` crate — its
stream is not stable across versions, which would silently break the committed
sample's reproducibility after a routine bump.

Output is a portable, streaming **JSONL** form (`dataset::io`) that round-trips
the logical model (`Node`/`Edge`) exactly — the interchange format until the
on-object storage writers (SPIKE-0003) land, after which the same generator can
feed those writers directly. Edge weights are quantised to 6 decimals so the
JSON text round-trip is bit-exact (serde_json's shortest-float writer otherwise
drifts a ULP; regression-guarded). A tiny (~6 KB) generated sample
(`tests/fixtures/sample_graph.jsonl`) is committed and pinned by an integration
test so it can never silently drift from the generator; large graphs are
regenerated into the gitignored `data/`, never committed. `serde_json` was
promoted from a dev- to a normal dependency (already license-clean and in the
manifest / Cargo.lock). Docs updated in `docs/process/datasets.md`.

**Diff size note:** ~1.5k lines, but the non-test/non-fixture source is modest
(rng ≈80, generator impl ≈230, io impl ≈140, cli impl ≈110); the remainder is
unit tests, the pinned sample fixture, and docs. The four parts (PRNG →
generator → portable IO → CLI) are one coherent feature — splitting would yield
half-working PRs — so it ships as one.

## Test evidence

- `cargo nextest run`: **180 tests run, 180 passed, 0 skipped** (38 of them in
  the dataset module: 8 PRNG, 17 generator, 8 IO + round-trip, 8 CLI, plus 3
  committed-sample integration tests in `tests/dataset_sample.rs`).
- `cargo test --doc`: 3 passed.
- `./format_code.sh`: green (cargo fmt + `clippy --workspace --all-targets
  --all-features -D warnings` + the standalone `formal/latency-sim` workspace +
  taplo). No `#[allow]`s added.
- Key behavioural checks proven by tests:
  - **Determinism:** identical seed ⇒ byte-identical JSONL
    (`output_is_byte_identical_for_a_fixed_seed`); verified at scale too —
    two independent 50k-node/500k-edge runs produced identical SHA-256.
  - **Power-law tail:** `degree_distribution_is_power_law_with_super_nodes`
    asserts a hub with in-degree ≫ 10× the mean and a long low-degree tail; at
    50k/500k scale the top hub absorbed ~44k edges vs. a mean degree of 10.
  - **Round-trip fidelity:** `larger_graph_round_trips_bit_exact_including_float_weights`
    and `committed_sample_matches_the_generator_byte_for_byte`.
  - **Scale:** 100k nodes / 1M edges generated in 1.49 s with 3.8 MB peak RSS
    (`/usr/bin/time -l`), confirming the `O(node_count)` memory design.
- **Coverage:** `cargo llvm-cov` not runnable in this sandbox
  (`llvm-tools-preview` not installed); measured in CI. Every public function
  and error branch in `rng`/`generator`/`io`/`cli` has a direct test, so the new
  module is densely covered; coverage is not expected to regress.
- License-clean: **no new dependencies** beyond promoting the already-recorded
  `serde_json`; `license_manifest` test passes. No third-party data committed
  (only a generated 6 KB sample); gitleaks pre-commit clean.

## Review gate

- [ ] adversarial-reviewer sign-off (see docs/process/adversarial-review-loops.md)
- [ ] premortem-analyst sign-off (see docs/process/adversarial-review-loops.md)
- [ ] `./format_code.sh` green
- [ ] `cargo nextest run` green (or `cargo test` outside Nix shell)
- [ ] coverage not regressed
- [ ] board item updated to `in_review`

<!-- Reviewers: append your verdict block below this line per adversarial-review-loops.md -->
