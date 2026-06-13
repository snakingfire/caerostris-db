# Commander's Intent — caerostris-db

> This is the north star. Every agent reads this first. When a rule elsewhere is
> silent or ambiguous, decide in the direction that best serves this intent.
> When you are about to do something this document would not endorse, stop.

## Mission

Build, from the ground up in Rust, an **ACID-compliant, transactional graph
database** that stores all durable state on **commodity object storage (S3)**,
runs **embedded (DuckDB-style) or as a server**, supports **single-writer /
multi-reader** concurrency, speaks the **entirety of openCypher**, and can answer
a **6-hop unanchored property-filtered match over a 1B-node / 10B-edge graph from
a cold start with P99 ≤ 1 s** — a target whose feasibility is **formally proven
before implementation**, not discovered afterward.

## End state (what "done" looks like at T0+4h)

A working engine that, graded against [`master-rubric.md`](requirements/master-rubric.md),
scores green across:

1. **ACID transactions** on S3-backed storage, with a **machine-checked proof**
   (TLA+/Apalache) of atomicity and isolation under single-writer/multi-reader.
2. A **custom storage format + commit protocol** designed for object storage:
   few, large, parallelizable range reads; commit = atomic manifest swap.
3. The **latency selectivity-envelope theorem** (below) proven *and* an analytical
   cost model + simulation that demonstrates the cold-start P99 ≤ 1 s target is
   reachable for any query inside the envelope, validated against a local S3
   mock with injected latency (and against real S3 when credentials arrive).
4. **100% of openCypher** — measured by the official TCK pass-rate climbing to
   100%. Phased delivery is allowed; a permanent subset is not.
5. **Pluggable secondary indices** (B-tree on text properties first), with the
   index interface designed for future index types.
6. **Fast aggregates** (count / sum / distinct) that exploit the storage layout.
7. **All four attach modes**: embedded writer-master, embedded read-only,
   embedded on a master-less database, and **server mode** (server is the writer
   master and also serves reads). Concurrent readers throughout.
8. **Python embedded bindings.**
9. **Resource-aware, configurable, optional caching** — warm queries faster, but
   the cold-start SLA must hold **without** the cache. Cache is never a crutch.
10. **≥90% test coverage**, integration tests against a local S3 mock, and
    criterion performance benchmarks.

## The one technical invariant nobody may quietly break: the latency theorem

The P99 ≤ 1 s target is **not** achievable for an arbitrary 6-hop unanchored
query — the physics forbid it. At 50 Mbps a 1 s budget permits only ~4 MB of S3
reads; at 1 Gbps, ~75 MB (after reserving compute + S3 round-trip latency). An
unconstrained degree-10 6-hop expansion touches 10⁶+ paths = hundreds of MB to
GB. **No storage layout makes that fit.**

It *is* achievable as a **conditional theorem**: a selective node-property
filter + a secondary index effectively *anchors* the search to a small seed set;
bounded frontier expansion + `LIMIT`-driven early termination keep
**bytes-read ≤ B_max** within **K** round-trip phases. The engine must:

- **Define the envelope precisely** (selectivity bound, byte budget B_max, phase
  bound K) as a first-class spec artifact — `EPIC-003` / `TASK-001`.
- **Prove** that any query inside the envelope hits the SLA (cost model + sim).
- **Detect** queries outside the envelope and handle them **explicitly** (reject
  with a clear error, warn, or degrade gracefully) — **never silently miss the
  SLA**.

If you find work drifting toward "we'll hit P99 only with a warm cache" or "only
if the data happens to be laid out luckily," that is a **falsification of the
design** — escalate to the steering committee immediately.

## Behaviour under uncertainty (this is an autonomous, agile, parallel run)

- **Make maximal parallel progress.** Spec, research, and implementation proceed
  **at the same time**. Start building the things that are clearly defined while
  still designing the things that are not. Do not stall a whole epic waiting for
  one open question.
- **Decompose, don't block.** If a task is ambiguous, split it: the clear part
  becomes a *ready* task; the unclear part becomes a *spike* with a design/research
  owner. File both on the board.
- **Adjudicate autonomously.** Decisions are made by the responsible agent, recorded
  in `.project/decisions/`, and — if design-level — ratified by the steering
  committee via the adversarial loop. There is no human in the loop until T0+4h.
  Record the decision, the alternatives, and *why*; move on.
- **Watch the wallclock.** Everyone is accountable to the pace in
  [`.project/pace/deadline.md`](../.project/pace/deadline.md). If you are behind,
  cut scope toward the rubric's highest weights, not quality of what ships.
- **Quality gates are not optional.** Every design clears adversarial
  falsification + steering sign-off; every code change clears adversarial review +
  pre-mortem sign-off. Fast, but never skipped. See
  [`process/adversarial-review-loops.md`](process/adversarial-review-loops.md).
- **Leave the campsite better.** Keep `CLAUDE.md`, the board, ADRs, specs, and
  agent memory current as you go — see
  [`process/memory-and-docs-policy.md`](process/memory-and-docs-policy.md).

## Hard constraints

- **Open source. No secrets, no private data, ever.** See
  [`process/open-source-guardrails.md`](process/open-source-guardrails.md).
- **License-clean only.** Every dependency and every dataset must carry a
  permissive/compatible license, verified and recorded.
- **No destructive git** (`reset --hard`, `push --force`, branch deletion) without
  explicit authorization for that exact action.
- **`./format_code.sh` is green before every landing.** Clippy warnings are errors.
- The cold-start SLA must hold **without** the local cache enabled.

## Definition of victory

At **T0+4h** the human (Jonas) can pull the repo, run the test + benchmark +
proof suites, read the latest rubric grade in `.project/reports/`, and find a
graph database that does what this document says — with the gaps (if any) named
honestly on the board, not hidden.
