# Core Requirements — caerostris-db

> Narrative companion to [`master-rubric.md`](master-rubric.md). The rubric is
> *graded*; this document *explains* each requirement and pins down the precise
> reading the swarm must build to. Where this and the rubric diverge, the rubric's
> measurable criteria win — fix this doc.

## R1. Engine shape

- **Graph database**: property-graph model (nodes, directed/typed edges,
  properties on both). openCypher semantics (Cat. 4) define the data model edges.
- **Embedded or server**, from one codebase:
  - **Embedded** (DuckDB-style): the engine is a library linked into the host
    process (Rust, and via bindings, Python — R8).
  - **Server**: a process that owns the writer role and serves queries to clients.
- **Written from scratch in Rust.** No embedding an existing graph/SQL engine.

## R2. ACID, transactional, single-writer / multi-reader

- **Atomicity, Consistency, Isolation, Durability** for committed transactions.
- **Single writer** ("master") per database at a time; **many concurrent readers**.
- A reader sees a **consistent snapshot**; a commit is **all-or-nothing** and
  **durable on ack**. Snapshot isolation is the floor; stronger is welcome if it
  doesn't cost the SLA.
- The single-writer constraint is *acceptable and intended* — it is what makes the
  S3 commit protocol tractable. Do not add multi-writer; do make reader scaling great.

## R3. Attach modes (all four)

A database lives in an object-store bucket/prefix. Clients attach as:

1. **Embedded writer-master** — this process is the sole writer + reads locally.
2. **Embedded read-only** — read-only; some *other* process is the writer-master.
3. **Embedded, master-less** — read-only against a DB with no live writer.
4. **Server mode** — the server is the writer-master *and* serves reads to remote
   clients. A DB may simultaneously have a server (writer+reader) and embedded
   read-only clients.

Writer coordination (leasing/fencing on the object store) must prevent two
writer-masters (split-brain). Readers never need the writer to be alive.

## R4. Object-storage-native storage format (custom)

No existing format meets the needs; design one. Requirements:

- **All durable state on S3-compatible object storage.** No reliance on a POSIX
  filesystem for durability (local disk may be used only as an *optional* cache, R9).
- Layout optimized for **few, large, parallelizable range GETs**, not many small
  random GETs — because S3 per-request latency, not bandwidth, dominates cold start.
- **Commit = atomic swap** of a manifest/root pointer; old versions remain readable
  until GC; readers pin a version.
- Columnar / sorted / adjacency structures that let the planner read only the
  bytes a query needs (serves R5 indices, R6 aggregates, and the latency envelope).
- Versioned + garbage-collectable; self-describing; forward-thinking about schema.

## R5. Pluggable secondary indices

- A **trait/interface** for secondary node indices, with a **B-tree on text
  properties** as the first concrete implementation, used by the planner for
  selective filtering (this is what *anchors* the unanchored query — see R7).
- Designed so future index types (range, full-text, composite, spatial, …) plug in
  without rewriting the core. Prove extensibility by stubbing one more type.

## R6. Fast aggregates

- `count`, `sum`, `distinct` (and the openCypher aggregations generally) must
  exploit the layout — e.g. maintained counts, columnar scans, distinct via sorted
  runs — not degrade to full-graph traversal where avoidable.

## R7. Latency: the selectivity-envelope theorem (read this twice)

The headline workload: **6-hop unanchored `MATCH` with node-property filter(s),
`LIMIT 10`, over 1B nodes / 10B edges, cold start, P99 ≤ 1 s** (2 s ceiling),
end-to-end at the client, on 1 Gbps; *ideally also* tolerable at 50–60 Mbps.

**This is provable only conditionally.** The deliverable is a formal artifact that:

1. **Defines the envelope.** Parameters: filter selectivity `s` (fraction of nodes
   passing), resulting seed-set size, per-hop fan-out bound, byte budget `B_max`,
   round-trip phase bound `K`. Derive `B_max` from bandwidth × (latency budget −
   K·L_p99 − compute). (At 1 Gbps ⇒ ~75 MB; at 50 Mbps ⇒ ~4 MB — the binding case.)
2. **Proves in-envelope queries hit the SLA**, via an analytical cost model +
   a discrete-event simulation calibrated to real S3 latency distributions.
3. **Specifies out-of-envelope handling**: detect at plan time (estimated bytes /
   fan-out exceed budget) and **reject / warn / degrade explicitly** — never
   silently blow the SLA.

Mechanism that makes in-envelope queries fast: a selective filter + secondary
index (R5) yields a tiny seed set ⇒ the "unanchored" search is *effectively
anchored* ⇒ bounded frontier expansion + `LIMIT`-driven early termination keep
bytes-read ≤ `B_max` within `K` parallel range-GET phases. **The cache (R9) must
not be required** to hit this.

## R8. Python embedded bindings

- Embedded Python API over the engine: open/attach (all R3 modes), run
  parameterized openCypher, ingest data, get results as native Python objects.
- Packaged (e.g. PyO3 + maturin) and tested with pytest in CI.

## R9. Caching — resource-aware, configurable, optional

- A local cache (memory and/or disk) may speed **warm** queries.
- It must be **configurable** (size, eviction, on/off) and **resource-aware**
  (respects a budget; doesn't OOM the host).
- It is **never required**: the cold-start SLA (R7) holds with the cache disabled.
  A test must prove this.

## R10. openCypher — the entirety

- Support **100% of openCypher**, measured by the **official TCK** pass-rate.
- **Phased delivery is allowed** (P1 reads → P2 writes+txns → P3 full breadth),
  and the swarm throws as many coding+testing agents at the long tail as needed —
  but the **acceptance bar is 100%**, not a curated subset. Track the live pass-rate.

## R11. Formal verification

- **Atomicity + isolation** of the commit/concurrency protocol: a **TLA+/Apalache**
  model, model-checked, kept in sync with the implementation.
- **Latency**: the R7 cost model + simulation.
- "Formally provable **before** any engine code" means: the latency envelope model
  (R7) and the commit-protocol model (R11) are committed and steering-ratified
  **before** the corresponding implementation tasks move to `in_progress`.

## R12. Quality, testing, process

- **≥90% line coverage** (cargo-llvm-cov), integration tests on a **local S3 mock**
  (MinIO/moto), and **criterion benchmarks** for the headline query + aggregates.
- **Hourly releases** for parallel testing while implementation continues.
- ADRs, specs, progress reports, and decision logs committed to git.
- **Open source**: no secrets/data; license-clean dependencies and datasets.

## Non-goals (YAGNI)

- Multi-writer / distributed consensus among writers (single-writer is by design).
- A bespoke query language (openCypher only).
- A custom object store (use S3-compatible storage; mock locally).
- GUI / web console (CLI + bindings suffice for the deadline).
