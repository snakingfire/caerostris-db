# Testing and Benchmarks — caerostris-db

> **Rubric anchor:** Cat. 10 (Tests/coverage/benches) is a **[GATE]** at weight 8.
> The grader reads `cargo-llvm-cov` output and checks for integration suite presence
> and criterion benchmark results. A green Cat. 10 requires ≥90% line coverage,
> integration tests on a local S3 mock, and tracked criterion benchmarks.
> See [`../requirements/master-rubric.md`](../requirements/master-rubric.md).

---

## Principles

- Tests are **first-class deliverables**. Every task moves to *done* only when it
  ships tests that keep coverage non-regressed. See
  [`autonomous-operating-model.md`](autonomous-operating-model.md).
- The `test-author` agent is responsible for driving coverage to ≥90% and keeping
  the TCK pass-rate climbing. The `perf-engineer` agent owns criterion suites and
  latency validation on the mock.
- All test commands below are **copy-pasteable** from the Nix `devenv` shell or any
  environment with the `rust-toolchain.toml` toolchain active.

---

## 1. Unit tests and doctests

Standard Rust unit tests live in `#[cfg(test)]` modules co-located with the code
they test. Every public function and non-trivial private function gets at least one
test. Doctests in doc comments count toward coverage and serve as executable API
examples — write them liberally.

```bash
# Run unit tests + doctests
cargo test

# Run with nextest (faster parallelism; preferred in the Nix shell)
cargo nextest run
```

Clippy runs in CI as a hard gate; treat every warning as a compile error locally:

```bash
cargo clippy --all-targets -- -D warnings
```

---

## 2. Property-based tests (proptest)

The commit protocol and storage format are the most critical invariants in the
system. Property-based testing with [`proptest`](https://docs.rs/proptest) is
**mandatory** for these subsystems — not optional.

### What must be property-tested

- **Commit protocol atomicity:** for any sequence of concurrent reads and a
  single writer commit, no reader sees a partial write. Properties: manifest swap
  is all-or-nothing; stale readers pin a consistent prior version; crash mid-commit
  leaves the DB in the previous consistent state.
- **Storage format round-trips:** for any generated graph (arbitrary nodes, edges,
  property bags), write → read returns byte-identical data; range-GET slices decode
  correctly; GC does not delete pinned versions.
- **Transaction isolation:** concurrent snapshot readers never observe each other's
  in-progress writes; committed data is always visible to readers that start after
  the commit.

### Location

```
tests/
  property/
    commit_protocol.rs
    storage_roundtrip.rs
    snapshot_isolation.rs
```

### Running

```bash
cargo nextest run --test property
# or equivalently:
cargo test --test property
```

Proptest persists failing examples in `.proptest-regressions/` — **commit these
files** so regressions are reproducible across agents.

---

## 3. Integration tests against a local S3-compatible mock

### Why a mock

All durable state lives on S3 (R4 / Cat. 2). Integration tests must exercise the
real `object_store` code path against an S3-compatible server running locally so
no AWS credentials are needed in CI.

### Approved mock implementations

- **MinIO** — high-fidelity S3-compatible server, runs as a single binary or
  Docker container. Preferred for full fidelity.
- **moto (Python `moto[s3]` server)** or **LocalStack** — acceptable alternatives,
  particularly for cross-language tests involving the Python bindings (Cat. 8 /
  R8).

Only one mock needs to be running at a time. CI uses MinIO by default. The env-var
contract below is the same regardless of which mock is active.

### Environment variable contract

| Variable | Required | Example | Meaning |
|---|---|---|---|
| `CAEROSTRIS_S3_ENDPOINT` | yes (integration tests) | `http://127.0.0.1:9000` | S3-compatible endpoint URL |
| `CAEROSTRIS_S3_REGION` | yes | `us-east-1` | Region string (MinIO accepts any) |
| `CAEROSTRIS_S3_BUCKET` | yes | `caerostris-test` | Bucket to use for test data |
| `AWS_ACCESS_KEY_ID` | yes | `minioadmin` | Access key (mock or real) |
| `AWS_SECRET_ACCESS_KEY` | yes | `minioadmin` | Secret key (mock or real) |
| `CAEROSTRIS_S3_FORCE_PATH_STYLE` | no | `true` | Force path-style URLs (needed for MinIO) |

When `CAEROSTRIS_S3_ENDPOINT` is **not set**, integration tests that require S3
are **skipped** (not failed). This lets `cargo test` run cleanly in environments
without a mock. CI always sets this variable.

### Tests directory layout

```
tests/
  integration/
    mod.rs              # shared setup: start mock, create bucket, teardown
    ingest.rs           # bulk-ingest a fixture graph → verify stored objects
    commit_roundtrip.rs # write txn → read in new reader session
    multi_reader.rs     # writer + N concurrent readers, snapshot isolation
    crash_recovery.rs   # kill writer mid-commit → DB opens cleanly in prior state
    gc.rs               # GC releases unreferenced objects, keeps pinned versions
    query_6hop.rs       # end-to-end: ingest fixture → 6-hop query → correct result
  property/             # (see §2)
  fixtures/
    tiny_graph.json     # hand-crafted 10-node/20-edge fixture (committed)
    README.md           # notes on each fixture; large datasets are NOT here
```

Large datasets are **never committed** — see [`datasets.md`](datasets.md) and
[`open-source-guardrails.md`](open-source-guardrails.md).

### Running integration tests (self-provisioned, parallel-safe)

**There is no manual setup step** — the swarm provisions its own shared S3 mock
and isolates every agent's data. The full contract is in
[`parallel-execution-and-environment.md`](parallel-execution-and-environment.md);
the short version a test (or a human) runs:

```bash
scripts/env/up.sh                             # idempotent: starts the shared mock if not already up
source "$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")/.project/env/local.env"
eval "$(scripts/env/bucket.sh integration)"   # unique CAEROSTRIS_S3_BUCKET + CAEROSTRIS_S3_PREFIX
cargo nextest run --test integration          # uses the env vars sourced above
```

Many agents run this **concurrently and safely**: one shared mock (no port wars),
a per-item bucket/prefix (no data cross-talk), worktree-isolated source. `up.sh`'s
provision ladder is **Docker MinIO → `moto_server` → `pip install moto[server]`
→ in-process memory backend** (unit-only). Under the hood the Docker path is
equivalent to:

```bash
docker run -d --name caerostris-minio -p 9000:9000 \
  -e MINIO_ROOT_USER=minioadmin -e MINIO_ROOT_PASSWORD=minioadmin \
  quay.io/minio/minio server /data    # minioadmin = MinIO's PUBLIC local-mock default, never a real secret
```

`integration/mod.rs` shared setup follows the same rule: call `up.sh`
(idempotent), source `local.env`, allocate a unique bucket/prefix, and tear down
its own namespace — never assume a clean shared bucket.

### Same code path against real S3

The `object_store` crate abstraction means integration tests run unmodified
against real S3 — just swap the env vars to real AWS credentials and a real
bucket. No test code changes. When AWS credentials arrive (from an environment
variable or instance role — **never from the repo**), the CI pipeline can be
extended with an additional job that sets `CAEROSTRIS_S3_ENDPOINT` to the real
endpoint and runs the same integration suite.

---

## 4. Coverage — ≥90% line coverage with cargo-llvm-cov

### Requirement

Cat. 10 is a **[GATE]**: ≥90% line coverage, measured by `cargo-llvm-cov`, run in
CI. The grader reads its output to produce the Cat. 10 score. Falling below 90% is
a P0 gap-closing task for the `test-author` agent.

### Commands

```bash
# Install (one-time):
cargo install cargo-llvm-cov

# Run all tests and report coverage (stdout summary):
cargo llvm-cov nextest

# Generate an HTML report (opens in browser):
cargo llvm-cov nextest --html --open

# Generate LCOV for CI artifact upload:
cargo llvm-cov nextest --lcov --output-path lcov.info

# Run only integration tests under coverage (requires mock running):
CAEROSTRIS_S3_ENDPOINT=http://127.0.0.1:9000 \
CAEROSTRIS_S3_REGION=us-east-1 \
CAEROSTRIS_S3_BUCKET=caerostris-test \
AWS_ACCESS_KEY_ID=minioadmin \
AWS_SECRET_ACCESS_KEY=minioadmin \
CAEROSTRIS_S3_FORCE_PATH_STYLE=true \
cargo llvm-cov nextest --test integration
```

### CI contract

CI runs `cargo llvm-cov nextest --lcov --output-path lcov.info` with the mock
running, then uploads `lcov.info` as an artifact. The rubric-grader reads the
summary line from the CI log. Coverage < 90% fails the Cat. 10 gate and triggers
an automatic gap-closing task.

---

## 5. Criterion performance benchmarks

### Location

```
benches/
  query_6hop.rs         # headline: 6-hop unanchored property-filtered MATCH, LIMIT 10
  aggregates.rs         # count / sum / distinct over the fixture graph
  ingest_throughput.rs  # bulk-ingest rate (nodes+edges per second)
  storage_io.rs         # range-GET throughput vs. object size
```

### Running

```bash
# Run all benches (default: 10-sample warm-up):
cargo bench

# Run only the headline 6-hop bench:
cargo bench --bench query_6hop

# Save a baseline for comparison:
cargo bench --bench query_6hop -- --save-baseline main

# Compare against a saved baseline:
cargo bench --bench query_6hop -- --baseline main
```

### Tracking results over time

Criterion writes results to `target/criterion/`. **Do not commit the raw output**
(`target/` is gitignored). Instead, the `perf-engineer` agent extracts the
headline numbers (mean latency, throughput) and appends them to
`.project/reports/benchmark-history.jsonl` each time benches run — one JSON object
per benchmark per run, with a timestamp and the git SHA. The rubric-grader reads
this file for Cat. 10 evidence.

### Benchmark fixtures

Benchmarks run against the **tiny fixture** (committed in `tests/fixtures/`) for
correctness + fast CI, and optionally against larger LDBC SNB-generated graphs
(see [`datasets.md`](datasets.md)) when a mock is running and `CAEROSTRIS_BENCH_DATASET`
is set to the path of the loaded dataset. Benchmarks skip the large-dataset path
gracefully when the variable is not set.

---

## 6. TCK harness for openCypher (Cat. 4)

### What the TCK is

The official openCypher Technology Compatibility Kit is a suite of Cucumber/Gherkin
`.feature` scenario files published at
<https://github.com/opencypher/openCypher/tree/master/tck/features>. Cat. 4 score
= TCK pass-rate %, with a floor of 0 if the harness is not wired.

### Runner design

A dedicated test crate at `tck-runner/` implements a Cucumber runner (using the
`cucumber` Rust crate) that:

1. Downloads or references the TCK feature files (via a Git submodule at
   `tck/openCypher` or a downloaded snapshot, **not committed verbatim** if large).
2. Implements the Gherkin step definitions to drive the caerostris-db engine
   (open a temp DB, execute openCypher statements, assert results).
3. Reports results in a machine-readable format (JUnit XML or JSON) to
   `.project/reports/tck-results-<timestamp>.json`.

```
tck-runner/
  Cargo.toml
  src/
    main.rs       # cucumber runner entry point
    steps/        # step definitions (Given/When/Then mappings)
    harness.rs    # engine lifecycle: temp DB per scenario, teardown
```

### Running

```bash
# Run the full TCK and print a pass-rate summary:
cargo run -p tck-runner

# Run a specific feature file:
cargo run -p tck-runner -- tck/openCypher/features/clauses/match/Match1.feature

# Run with JSON output for the grader:
cargo run -p tck-runner -- --format json --output .project/reports/tck-latest.json
```

### CI contract

CI runs the full TCK and fails if the pass-rate regresses from the previous run.
The rubric-grader reads `.project/reports/tck-latest.json` and sets the Cat. 4
score to the pass-rate percentage. A pass-rate below the current phase target is a
P0 gap-closing task.

### Phased delivery

- **Phase 1 (P1):** read-only clauses (`MATCH`, `RETURN`, `WHERE`, `WITH`, `ORDER
  BY`, `LIMIT`, `SKIP`, `OPTIONAL MATCH`, `UNWIND`).
- **Phase 2 (P2):** write clauses + transactions (`CREATE`, `SET`, `DELETE`,
  `MERGE`, `REMOVE`, explicit transaction control).
- **Phase 3 (P3):** full breadth (all remaining TCK scenarios).

The harness marks P2/P3 scenarios as *pending* (not *failed*) until the engine
supports them, so the pass-rate reflects only wired scenarios and the pending count
is tracked separately.

---

## 7. Latency benchmark with injected S3 latency

### Purpose

The perf-engineer validates the selectivity-envelope SLA (Cat. 3 / R7) on the mock
by injecting realistic per-request S3 latency so the mock behaves like real S3.
This is the primary mechanism for proving "P99 ≤ 1 s cold, without cache" before
real AWS credentials arrive.

### Latency injection approach

MinIO does not natively inject per-request latency. Use one of:

1. **`tc netem`** (Linux): add artificial delay to the loopback interface used by
   the mock.
   ```bash
   # Add 5 ms ± 1 ms latency to all loopback traffic (simulate fast-region S3):
   sudo tc qdisc add dev lo root netem delay 5ms 1ms distribution normal
   # Remove:
   sudo tc qdisc del dev lo root
   ```

2. **Toxiproxy**: a programmable TCP proxy that sits between the engine and MinIO,
   adding configurable latency, jitter, and packet loss. Preferred for
   reproducibility across platforms.
   ```bash
   # Start Toxiproxy:
   toxiproxy-server &
   # Create a proxy (MinIO at 9000, exposed at 9100, with 10ms latency):
   toxiproxy-cli create s3-mock -l 127.0.0.1:9100 -u 127.0.0.1:9000
   toxiproxy-cli toxic add s3-mock -t latency -a latency=10 -a jitter=2
   # Point the engine at port 9100 instead of 9000:
   export CAEROSTRIS_S3_ENDPOINT=http://127.0.0.1:9100
   ```

### Standard latency scenarios

The perf-engineer runs the 6-hop benchmark under each of these injection profiles
and records results to `.project/reports/benchmark-history.jsonl`:

| Profile | Per-request latency | Jitter | Models |
|---|---|---|---|
| `loopback` | ~0 ms | – | raw engine throughput (no network) |
| `fast-s3` | 5 ms | 1 ms | same-region S3 on a fast box |
| `nominal-s3` | 20 ms | 5 ms | typical cross-AZ S3 |
| `slow-s3` | 50 ms | 10 ms | conservative / degraded S3 |

The SLA target (P99 ≤ 1 s) must hold under `nominal-s3` and should hold under
`slow-s3` for queries inside the selectivity envelope. Failure under `nominal-s3`
is a P0 Cat. 3 gap.

### Benchmark command

```bash
# With Toxiproxy injecting nominal-s3 latency (20ms ± 5ms):
CAEROSTRIS_S3_ENDPOINT=http://127.0.0.1:9100 \
CAEROSTRIS_S3_REGION=us-east-1 \
CAEROSTRIS_S3_BUCKET=caerostris-test \
AWS_ACCESS_KEY_ID=minioadmin \
AWS_SECRET_ACCESS_KEY=minioadmin \
CAEROSTRIS_S3_FORCE_PATH_STYLE=true \
cargo bench --bench query_6hop -- --save-baseline nominal-s3
```

Record the P99 latency number from the criterion output in
`.project/reports/benchmark-history.jsonl` with `latency_profile: "nominal-s3"`.
The rubric-grader reads this for Cat. 3 evidence.
