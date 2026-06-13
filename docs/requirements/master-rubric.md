# Master Requirements Rubric — caerostris-db

> **The single graded source of truth.** Every 20 minutes the `rubric-grader`
> scores the project against this file, commits a report to `.project/reports/`,
> and files gap-closing tasks. Every epic and task on the board carries a
> `rubric_refs` field pointing back here. If a requirement is not in this rubric,
> it is not a requirement. If it is here, it ships or it is named as a gap.

## How grading works

- Each **category** has a **weight** (sums to 100) and is scored **0–100**.
- **Overall = Σ(category_score × weight) / 100.**
- Scores are **evidence-based**: a score claim must cite the artifact proving it
  (a passing test, a benchmark number, a committed proof, a TCK pass-rate). No
  evidence → score it ≤ 25 ("asserted, unverified").
- Each category lists **score anchors** (what 0 / 50 / 100 looks like). Interpolate.
- A category tagged **[GATE]** is a hard requirement: the project is not "done"
  while any GATE category is < 90, regardless of overall score.

## Targets & non-negotiables (apply across categories)

- **Latency:** 6-hop unanchored, node-property-filtered match, `LIMIT 10`, over
  1B nodes / 10B edges, **cold start, P99 ≤ 1 s (target), ≤ 2 s (hard ceiling)**,
  end-to-end as observed by the client, on a 1 Gbps box — *for queries inside the
  selectivity envelope* (see Cat. 3). Must hold **without** the local cache.
- **openCypher:** 100% of the spec (TCK pass-rate → 100%). Phased OK; subset not.
- **Storage:** all durable state on S3-compatible object storage. Commit is ACID.
- **No secrets/data in the repo. License-clean deps + datasets.**

---

## Categories

### Cat. 1 — ACID transactions & correctness  **[GATE]**  (weight: 14)
Single-writer/multi-reader transactions on S3-backed storage: atomic commit,
durable on ack, consistent reads, snapshot isolation (or stronger) for readers.
- **0:** no transactions / reads can see partial commits.
- **50:** atomic single-writer commit works; isolation informally argued; happy-path tests.
- **100:** commit protocol implemented + property-tested; readers get a consistent
  snapshot; crash/partial-write recovery tested; behaviour matches the TLA+ model (Cat. 11).

### Cat. 2 — Storage format & S3 commit protocol  **[GATE]**  (weight: 12)
Custom on-object layout designed for object storage: columnar/adjacency layout
enabling **few, large, parallel range GETs**; commit = atomic manifest/root swap;
versioned, GC-able.
- **0:** ad-hoc/row-per-object or no format.
- **50:** format spec written + writer/reader roundtrips a real graph; commit swaps a manifest.
- **100:** format documented in an ADR + spec; range-read access patterns implemented;
  manifest swap is atomic & concurrent-reader-safe; layout demonstrably serves the latency envelope.

### Cat. 3 — Latency: selectivity-envelope theorem + measured SLA  **[GATE]**  (weight: 14)
The envelope (selectivity bound, byte-budget B_max, phase bound K) is defined; a
proof/cost-model shows in-envelope queries hit P99 ≤ 1 s cold; out-of-envelope
queries are detected & handled explicitly; benchmarks corroborate.
- **0:** no model; SLA only "hoped for".
- **50:** envelope defined + analytical cost model committed; out-of-envelope detection designed.
- **100:** cost model + simulation (validated vs. S3 latency distributions) prove the target;
  benchmark on the mock (injected latency) meets it; out-of-envelope handling implemented & tested;
  no reliance on cache.

### Cat. 4 — openCypher completeness (TCK)  **[GATE]**  (weight: 12)
Live TCK pass-rate is the metric. Phased: P1 reads → P2 writes+txns → P3 full breadth.
- **Score = TCK pass-rate %**, with a floor: 0 if the TCK harness isn't wired.
- **`pass_rate = pass / total`, where `total = pass + pending + fail`.** Both
  `pending` (unimplemented) and `fail` (wrong result) are in the denominator;
  **no scenario is ever excluded from `total`**. Reaching 100 therefore requires
  `pending == 0 && fail == 0` — `pass_rate = pass / (pass + fail)` (excluding
  `pending`) is **forbidden**: it is a curated subset and falsifies "100% means
  all of it, not a subset". Moving a scenario to `pending` to inflate the rate is
  likewise forbidden. (BUG-0007 / Decision 0008.)
- **The suite is pinned.** "100% of the TCK" is defined against a pinned
  openCypher release tag (**`1.0.0-M23`**, commit `007895a`) with a recorded
  scenario `total` (**1615** scenarios across 220 `.feature` files). The harness
  emits the tag and `total` in its machine-readable output and a guard fails if
  the loaded scenario count differs from the recorded pin (catches silent suite
  shrinkage). The grader reads `pass/total` from `.project/reports/tck-latest.json`.
- **100:** the official openCypher TCK (pinned tag) passes 100% — i.e.
  `pass == total`, `pending == 0`, `fail == 0` — run in CI.

### Cat. 5 — Pluggable secondary indices  (weight: 7)
B-tree on text properties first; index trait/interface designed for future types
(e.g. range, full-text, spatial) without core rewrites.
- **0:** none. **50:** B-tree index built + used by the planner for filtering.
- **100:** pluggable index trait; B-tree impl; planner picks indices by selectivity;
  one additional index type stubbed against the same trait to prove extensibility.

### Cat. 6 — Fast aggregates  (weight: 5)
`count` / `sum` / `distinct` exploit the layout (e.g. metadata counts, columnar scans).
- **0:** none/naïve full-scan only. **50:** correct aggregates over the engine.
- **100:** layout-accelerated aggregates with benchmarks beating naïve scan.

### Cat. 7 — Concurrency & attach modes  **[GATE]**  (weight: 8)
All four modes: embedded writer-master; embedded read-only; embedded on a
master-less DB; server mode (server = writer-master + serves reads). Concurrent readers.
- **0:** single mode only. **50:** embedded writer + reader + server reads.
- **100:** all four modes work + tested; writer-leasing/coordination prevents split-brain;
  concurrent readers verified under load.

### Cat. 8 — Python embedded bindings  (weight: 6)
Pythonic embedded API (open/attach, query, ingest) over the engine.
- **0:** none. **50:** can open a DB + run a read query from Python.
- **100:** open/attach (all modes), parameterized queries, ingest, results as
  native Python objects; packaged + tested (pytest) in CI.

### Cat. 9 — Caching (resource-aware, optional)  (weight: 4)
Configurable, resource-aware local cache that speeds warm queries; the cold SLA
holds with the cache **off**.
- **0:** none or required-for-SLA. **50:** optional cache speeds warm queries.
- **100:** configurable size/eviction, resource-aware, measurably faster warm,
  and a test proving the cold SLA holds with caching disabled.

### Cat. 10 — Tests, coverage & benchmarks  **[GATE]**  (weight: 8)
≥90% line coverage; integration tests against a local S3 mock; criterion benches.
- **Score scales with coverage% and presence of integration + perf suites.**
- **100:** ≥90% coverage reported by cargo-llvm-cov in CI; integration suite on
  MinIO/moto; criterion benches for the headline query + aggregates, tracked over time.

### Cat. 11 — Formal verification artifacts  **[GATE]**  (weight: 6)
TLA+/Apalache model of the commit/concurrency protocol (atomicity + isolation);
the latency cost-model + simulation (Cat. 3 shares this).
- **0:** none. **50:** TLA+ model drafted; latency model committed.
- **100:** TLA+ model model-checked (no invariant violations) for the implemented
  protocol; latency proof/sim committed and referenced by the design; both kept in
  sync with the code (drift = a bug).

### Cat. 12 — Engineering & process health  (weight: 4)
Board hygiene, ADRs/specs/reports/decisions committed, hourly releases cut,
CLAUDE.md + agent memory current, no secrets, license-clean, CI green.
- **0:** chaos / undocumented / red CI. **50:** board used, CI green, docs committed.
- **100:** board reflects reality; ADRs for every major decision; ≥1 hourly release
  per hour; CLAUDE.md/memory auto-maintained; gitleaks clean; all deps+datasets license-verified.

---

## Scoreboard template (the grader fills this each cycle)

| Cat | Name | Weight | Score | Evidence (path / number) | Gate? |
|----:|------|------:|------:|--------------------------|:----:|
| 1 | ACID txns & correctness | 14 | – | | ✓ |
| 2 | Storage format & S3 commit | 12 | – | | ✓ |
| 3 | Latency envelope + SLA | 14 | – | | ✓ |
| 4 | openCypher (TCK %) | 12 | – | | ✓ |
| 5 | Secondary indices | 7 | – | | |
| 6 | Fast aggregates | 5 | – | | |
| 7 | Concurrency & attach modes | 8 | – | | ✓ |
| 8 | Python bindings | 6 | – | | |
| 9 | Caching | 4 | – | | |
| 10 | Tests/coverage/benches | 8 | – | | ✓ |
| 11 | Formal verification | 6 | – | | ✓ |
| 12 | Process health | 4 | – | | |
| | **OVERALL** | **100** | **–** | | |

**Done =** every `[GATE]` category ≥ 90 **and** overall ≥ 90 **and** Cat. 4 = 100.
