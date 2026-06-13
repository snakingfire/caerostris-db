# Datasets — caerostris-db

> **Rubric anchor:** Cat. 10 (Tests/coverage/benches) and Cat. 3 (Latency envelope
> + SLA). Correctness tests use small datasets; scale + latency validation uses
> large/generated ones. See [`../requirements/master-rubric.md`](../requirements/master-rubric.md).

---

## Principles

- **Local-first.** Every dataset is used locally against the S3 mock before any
  real S3 is involved. No test requires live AWS credentials.
- **License-clean only.** Before any dataset is ingested or committed (even a
  schema or subset), the `dataset-scout` agent **must verify the license is
  permissive/redistributable** and record the verification in the board item and in
  `.project/decisions/` as a decision-log entry. Datasets with unclear, GPL, or
  proprietary licenses are blocked until steering approves.
- **Never committed to git.** Raw graph data, converted storage files, and bulk
  downloads are all gitignored. Only hand-crafted tiny fixtures (≤ a few KB) may
  be committed. See [`.gitignore`](../../.gitignore) and
  [`open-source-guardrails.md`](open-source-guardrails.md).

---

## Candidate datasets

| # | Name | Approx size | Domain | License | Source URL |
|---|------|-------------|--------|---------|------------|
| 0 | Tiny hand-made fixture | 10 nodes / 20 edges | synthetic | n/a (authored here) | `tests/fixtures/tiny_graph.json` (committed) |
| 1 | ego-Facebook (SNAP) | 4k nodes / 88k edges | social | CC BY-NC 4.0 | https://snap.stanford.edu/data/ego-Facebook.html |
| 2 | ego-Twitter (SNAP) | 81k nodes / 1.8M edges | social | CC BY-NC 4.0 | https://snap.stanford.edu/data/ego-Twitter.html |
| 3 | Higgs-Twitter (SNAP) | 457k nodes / 14.9M edges | social/diffusion | CC BY-NC 4.0 | https://snap.stanford.edu/data/higgs-twitter.html |
| 4 | soc-Pokec (SNAP) | 1.6M nodes / 30.6M edges | social | CC BY-NC 4.0 | https://snap.stanford.edu/data/soc-Pokec.html |
| 5 | com-LiveJournal (SNAP) | 4M nodes / 34.7M edges | social communities | CC BY-NC 4.0 | https://snap.stanford.edu/data/com-LiveJournal.html |
| 6 | com-Friendster (SNAP) | 65M nodes / 1.8B edges | social | CC BY-NC 4.0 | https://snap.stanford.edu/data/com-Friendster.html |
| 7 | MovieLens 25M | ~200k nodes (movies+users) / 25M ratings-as-edges | recommendations | CC BY 4.0 | https://grouplens.org/datasets/movielens/25m/ |
| 8 | Wikidata truthy subset | varies (100M+ nodes in full) | knowledge graph | CC0 1.0 | https://dumps.wikimedia.org/wikidatawiki/ |
| 9 | LDBC SNB generated (sf1) | ~3M nodes / ~17M edges (scale factor 1) | social benchmark | Apache 2.0 (generator) | https://github.com/ldbc/ldbc_snb_datagen |
| 10 | LDBC SNB generated (sf10) | ~30M nodes / ~176M edges | social benchmark | Apache 2.0 (generator) | same as above |
| 11 | LDBC SNB generated (sf100) | ~300M nodes / ~1.7B edges | social benchmark | Apache 2.0 (generator) | same as above |
| 12 | LDBC SNB generated (sf1000) | ~3B nodes / ~17B edges | social benchmark | Apache 2.0 (generator) | same as above |
| 13 | Synthetic generator (custom) | configurable (default 1M nodes / 10M edges) | configurable, power-law | n/a (authored here, generated) | `src/dataset/` + `caerostris-db generate-dataset` (T-0035) |

### License verification requirement

For each dataset in the table above, the **`dataset-scout` agent must**:

1. Read the license text at the source URL (not just the license name).
2. Confirm the license permits use for software testing and benchmarking.
3. Check whether the license permits redistribution of derived data — if not, note
   that only locally-generated statistics (counts, latencies) may be published, not
   the raw or converted graph data.
4. Record the verification as a decision-log entry in
   `.project/decisions/NNNN-dataset-license-<name>.md` before the dataset is
   ingested.

**SNAP datasets (CC BY-NC 4.0):** non-commercial use only. Acceptable for an
open-source database engine's test suite. Redistribution of the raw data is not
permitted — do not commit the data or publish it as a release artifact.

**LDBC SNB generator (Apache 2.0):** the *generator code* is Apache 2.0. The
*generated data* is synthetic and carries no third-party rights. This is the
cleanest option for large-scale testing and for the 1B-node / 10B-edge headline
target.

---

## The 1B-node / 10B-edge headline target

No single static download reaches this size conveniently. The canonical path is:

1. **LDBC SNB generator at scale factor 1000 (sf1000):** generates ~3B nodes /
   ~17B edges. This exceeds the target and produces the data in a structured
   schema the engine can ingest. The generator is run once, outputs to local disk,
   and is then bulk-ingested.
2. **Custom synthetic generator** (`src/dataset/`, exposed as the
   `caerostris-db generate-dataset` subcommand): for configurable degree
   distributions, property cardinalities, and graph shapes that stress specific
   parts of the engine (e.g. high-degree hubs for the 6-hop worst case). This
   generator is written in Rust as part of the project — see
   [the synthetic generator section below](#the-synthetic-graph-generator-t-0035).

Small datasets (fixtures 0–5) are for **correctness** — fast to ingest, easy to
verify results by hand. Large and generated datasets (6–12) are for **scale + latency
validation** and for proving the selectivity-envelope SLA (Cat. 3 / R7).

---

## The synthetic graph generator (T-0035)

A built-in, **license-clean** graph generator (`src/dataset/`, rubric Cat. 10).
A *generated* graph carries **no external licence and no PII** — it is produced
from a seed, not downloaded — so it sidesteps the redistribution restrictions on
the third-party datasets above (the `dataset-scout` license-verification step
does not apply: there is no source licence to verify). It is the safest source
of a representative, scalable graph for benches and integration tests.

### What it produces

- **Configurable size** — default **1M nodes / 10M edges** (the headline
  target); any size via flags.
- **Labels + text properties** — nodes carry one or two labels and the text
  properties `name`, `bio`, `country` plus an integer `age`; edges are
  **directed, typed** (`KNOWS`, `FOLLOWS`, `WORKS_AT`, `LOCATED_IN`, `TAGGED`)
  with `weight` / `rank` properties.
- **Power-law in-degree with super-nodes** — targets are drawn by rank-Zipf
  sampling, so a few hubs absorb a large share of edges. This exercises the
  tail fan-out case the latency envelope must handle (SPIKE-0004). The
  `--zipf` exponent dials tail heaviness.
- **Deterministic given a seed** — identical `--seed` + size ⇒ byte-identical
  output on every platform (a vendored SplitMix64 PRNG; no `rand` dependency,
  whose stream is not stable across versions). Generation uses `O(node_count)`
  memory (the edge stream is constant-memory), so 10M edges generate in seconds
  with a few MB of RAM.

### Generating

```bash
# Headline 1M nodes / 10M edges to a (gitignored) data file:
mkdir -p data/synth
cargo run --release -- generate-dataset --out data/synth/headline.jsonl
# (defaults: --nodes 1000000 --edges 10000000 --seed 0 --zipf 1.0)

# A smaller, heavier-tailed graph to stdout (pipe into anything):
cargo run --release -- generate-dataset \
  --nodes 100000 --edges 1000000 --seed 7 --zipf 1.3
```

The output is **JSONL** (one self-describing JSON record per line: a `meta`
header, then `node` records, then `edge` records) — a portable form that round-
trips the logical model exactly and streams on both write and read. It is the
interchange format until the on-object storage writers (SPIKE-0003) land; the
same generator can then feed those writers directly. Load it back with
`caerostris_db::dataset::read_records`.

### Licence note

The generator is authored in this repository (MIT, like the rest of the crate).
**Generated graphs are synthetic** and carry **no third-party rights, no
external licence, and no PII** — they may be regenerated, published as
statistics, or shared freely. Large generated graphs are **not committed**
(they land under `data/`, which is gitignored, and are regenerated from a seed);
only a tiny (~6 KB) sample, `tests/fixtures/sample_graph.jsonl`, is committed,
and an integration test pins it to the generator so it can never silently drift.

---

## Local-first plan

### Step 1: download or generate

```bash
# SNAP ego-Facebook (small, for integration tests):
mkdir -p data/snap
cd data/snap
curl -O https://snap.stanford.edu/data/facebook_combined.txt.gz
gunzip facebook_combined.txt.gz

# LDBC SNB sf1 (benchmark development):
# Follow https://github.com/ldbc/ldbc_snb_datagen for generator setup.
# Output directory: data/ldbc/sf1/
```

All data lands under `data/` which is gitignored.

### Step 2: convert to caerostris-db storage format

A bulk-ingest CLI command reads standard graph formats (edge-list TSV, LDBC CSV,
Wikidata JSON-LD) and writes to the caerostris-db storage format:

```bash
# Ingest edge-list (SNAP format) into the local mock:
CAEROSTRIS_S3_ENDPOINT=http://127.0.0.1:9000 \
CAEROSTRIS_S3_BUCKET=caerostris-bench \
cargo run --release -- ingest \
  --format snap-edgelist \
  --input data/snap/facebook_combined.txt \
  --db s3://caerostris-bench/ego-facebook

# Ingest LDBC SNB sf1:
cargo run --release -- ingest \
  --format ldbc-snb \
  --input data/ldbc/sf1/ \
  --db s3://caerostris-bench/ldbc-sf1
```

### Step 3: run locally on the mock

With the dataset ingested, run correctness queries and criterion benchmarks against
the mock endpoint (see [`testing-and-benchmarks.md`](testing-and-benchmarks.md)).

### Step 4: bulk-ingest to real S3 for E2E (when credentials arrive)

When AWS credentials are available via the environment or instance role (never the
repo), point the ingest command at a real S3 bucket:

```bash
# Credentials come from the environment, not the codebase:
AWS_PROFILE=caerostris-bench \
cargo run --release -- ingest \
  --format ldbc-snb \
  --input data/ldbc/sf1000/ \
  --db s3://caerostris-prod-bench/ldbc-sf1000
```

The same code path runs; no changes to the ingest logic.

---

## Gitignore rules for data

The following patterns are in `.gitignore` and **must never be removed**:

```
/data/
/target/
*.bin
*.parquet
*.arrow
*.csv.gz
*.tsv.gz
```

Any agent that commits files under `data/` or commits large binary blobs is
triggering a P0 open-source-guardrails violation. See
[`open-source-guardrails.md`](open-source-guardrails.md).
