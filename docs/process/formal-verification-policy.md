# Formal Verification Policy — caerostris-db

> What must be formally established, what artifacts prove it, and — critically —
> what must exist before the corresponding implementation tasks move to
> `in_progress`. Formal verification is not a post-hoc audit; it is a
> prerequisite. Read this alongside
> [`../commanders-intent.md`](../commanders-intent.md) (the latency theorem and
> the proof mandate), [`../requirements/master-rubric.md`](../requirements/master-rubric.md)
> (Cat 11 and Cat 3), and [`steering-committee.md`](steering-committee.md)
> (ratification process).

## The ordering rule

**Model-checked and steering-ratified before the corresponding implementation
task moves to `in_progress`.** This is not aspirational. It is the operational
meaning of "formally provable before any line of code":

- The TLA+/Apalache commit-protocol spec must be model-checked (no invariant
  violations) and ratified by `steering-formal-methods` +
  `steering-distributed-acid` **before** any storage commit-path implementation
  task (`EPIC-002` tasks that touch the write path) moves past `backlog`.
- The latency cost model and discrete-event simulation must be committed and
  ratified by `steering-perf-sla` + `steering-formal-methods` **before** any
  query-execution implementation task (`EPIC-003`) moves past `backlog`.

Violation of the ordering rule is a process bug. File a `BUG-NNNN` on the
board and escalate to the steering committee.

---

## Artifact 1 — Commit / concurrency protocol (TLA+/Apalache)

**What it proves:** atomicity and snapshot isolation (minimum) of the S3-backed
commit protocol under single-writer / multi-reader with crash and
partial-write faults.

### Scope of the model

The model must cover:

- **Single-writer commit path:** a writer building a new manifest, performing
  object writes, and atomically swapping the manifest pointer. The invariant:
  a reader that observes the new manifest sees a complete, consistent snapshot;
  a reader that observes the old manifest also sees a complete, consistent
  snapshot. No reader ever sees a partial write.
- **Multi-reader concurrency:** N readers (N ≥ 2) may read concurrently with
  a writer and with each other. Readers obtain a snapshot at open time; the
  snapshot is stable for the duration of the read transaction.
- **Crash at any point:** a crash mid-commit must leave the database in either
  the old consistent state or the new consistent state — never a mix. Recovery
  is automatic on next open.
- **Partial write:** an object write that does not complete (network drop,
  partial upload) must be detected and not incorporated into any reader's
  snapshot.
- **Writer leasing / attach modes:** the model must demonstrate that two
  simultaneous writer claims cannot both commit (split-brain prevention).

### Tooling

TLA+ Toolbox or Apalache (model checker). The `formal-prover` agent maintains
the spec under `formal/commit-protocol/`. Apalache is preferred for bounded
model checking with type annotations; fall back to TLC for exhaustive state
space when the model is small enough.

### Keeping the model in sync

Drift between the TLA+ model and the implementation is a bug — file
`BUG-NNNN` immediately. The `formal-prover` agent reviews the model on every
commit that touches the write path and certifies sync in the PR description.
The rubric grader checks for a sync certification on every cycle; absence
downgrades Cat 11 to ≤ 50.

### Rubric mapping

- **Cat 11** (weight 6, GATE): score 100 requires model-checked (no invariant
  violations) + kept in sync with implementation.
- **Cat 1** (weight 14, GATE): score 100 requires "behaviour matches the TLA+
  model."

---

## Artifact 2 — Latency cost model + discrete-event simulation

**What it proves:** that every query inside the selectivity envelope hits
P99 ≤ 1 s cold start (target) / ≤ 2 s (hard ceiling) on a 1 Gbps box, end-to-end
as observed by the client, **without cache enabled**.

### Envelope parameters

The envelope must be defined as a first-class spec artifact (ADR, ratified by
`steering-perf-sla` + `steering-formal-methods`) before any simulation work
begins. Parameters:

| Symbol | Meaning | Ratified value (TBD in ADR) |
|--------|---------|----------------------------|
| `s` | Selectivity bound — fraction of nodes/edges the seed filter may return | TBD |
| `B_max` | Maximum bytes read across all S3 GETs for an in-envelope query | TBD |
| `K` | Maximum number of sequential S3 round-trip phases | TBD |

Any query that requires reading more than `B_max` bytes or more than `K`
sequential phases is **out-of-envelope** and must be detected and handled
explicitly (reject, warn, or degrade gracefully). Silently missing the SLA on
an out-of-envelope query is a correctness bug.

### Analytical cost model

A closed-form expression (or a set of inequalities) that, given `s`, `B_max`,
`K`, the S3 latency distribution parameters (median, P99 per GET), and the
network bandwidth `W`, yields the expected P99 query latency for an in-envelope
6-hop unanchored property-filtered match. This model must:

- Account for parallelizable vs. sequential GET phases.
- Show that the target is achievable (i.e., the inequality closes under the
  stated parameters).
- Be committed to `formal/latency-model/` as a readable document + derivation.

### Discrete-event simulation

A simulation (Rust or Python) that:

- Samples S3 latency from a realistic distribution (calibrated to AWS S3
  empirical P50/P99 numbers at the time of writing; updated when real
  credentials arrive).
- Models the query execution phases for a 6-hop unanchored match over
  1B nodes / 10B edges with the storage format's actual access pattern (large
  parallel range GETs, manifest fetch, index lookup).
- Runs ≥ 10 000 trials per parameter combination.
- Reports P50, P95, P99, and max latency for in-envelope queries with cache
  disabled.
- Demonstrates that P99 ≤ 1 s for the target parameter combination.
- Demonstrates P99 ≤ 2 s (hard ceiling) under the worst in-envelope case.

The simulation is committed to `formal/latency-sim/` and is runnable with a
single command (documented in its README).

### Calibration against the mock

Once the storage implementation exists, the simulation must be re-calibrated
against measured latency from the local S3 mock (MinIO/moto with injected
latency). The simulated and measured P99 must agree within a documented
tolerance. Discrepancy beyond tolerance is a bug in either the model or the
implementation — investigate before proceeding.

### Rubric mapping

- **Cat 3** (weight 14, GATE): score 100 requires cost model + simulation
  committed, validated against S3 latency distributions, benchmark on mock
  meets the target, out-of-envelope detection implemented and tested, no cache
  reliance.
- **Cat 11** (weight 6, GATE): the latency model is listed as a required formal
  artifact alongside the TLA+ model.

---

## What "ratified before in_progress" means operationally

The board item for a storage write-path implementation task carries in its
`deps` field the SPIKE/task IDs for the commit-protocol TLA+ spec and the
envelope ADR. The planner does not flip those implementation tasks from
`backlog` to `ready` until both dependencies are `done`. The `done` state of
a formal-verification task requires:

1. The artifact is committed to `formal/` under the correct path.
2. The model checker has been run (output committed or summarized in the ADR
   with no invariant violations reported).
3. The relevant steering members have ratified the ADR (see
   [`steering-committee.md`](steering-committee.md)).
4. The `adversarial-review-loops.md` design falsification loop has completed
   with `approve` verdicts.

Until all four conditions hold, dependent implementation tasks stay `backlog`.
This is the mechanism that makes "formally provable before any line of code"
operationally enforceable, not just aspirational.
