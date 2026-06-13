---
id: T-0000
title: Self-provision the local environment and guarantee parallel-safe isolation
type: task
status: in_progress
priority: P0
assignee: implementer-wf_365f5b82-d76-3
epic: EPIC-009
deps: []
rubric_refs: [10, 12]
estimate: M
created: T0
updated: T+1:24
---

## Context
The run must execute with **no human environment setup** and must let **many
agents work concurrently without conflicting**. The scaffolding ships the scripts
`scripts/env/up.sh`, `scripts/env/bucket.sh`, `scripts/env/down.sh` and the
contract in
[`docs/process/parallel-execution-and-environment.md`](../../../docs/process/parallel-execution-and-environment.md).
This task **verifies and hardens** that machinery and wires the test/build path to
use it. It is a foundational dependency: **T-0001 and every integration test
depend on it.**

## Acceptance criteria
- [ ] `scripts/env/up.sh` is idempotent and concurrency-safe — verified by
      invoking it from several processes at once (atomic lock; exactly one mock
      ends up running; the rest no-op). It writes `.project/env/local.env` in the
      **main** worktree (resolved via `git --git-common-dir`), so all worktrees
      share one endpoint.
- [ ] The provision ladder works end to end on this host (Docker MinIO → moto →
      `pip install moto[server]` → in-process memory marker). If a server backend
      can't be obtained, it exits non-zero with a clear message (no silent
      pretend-coverage).
- [ ] `scripts/env/bucket.sh <ID>` yields a unique, valid bucket+prefix per work
      item; two different IDs never collide; safe to call repeatedly.
- [ ] The integration test harness (`tests/integration/mod.rs` shared setup)
      calls `up.sh` (idempotent) + `bucket.sh`, runs against the isolated
      namespace, and tears it down — never assuming a clean shared bucket.
- [ ] Demonstrated: **N integration tests run concurrently with zero cross-talk**
      (e.g. a CI/nextest run with parallelism > 1, or a scripted parallel proof).
- [ ] `mainspring` orient ensures env up each epoch; the `pace-marshal` cron
      re-provisions if the mock died — both verified.
- [ ] `docs/process/parallel-execution-and-environment.md` matches the
      implementation (fix the doc in the same change if it drifts).
- [ ] `./format_code.sh` green.

## Notes / log
Scripts authored at scaffold time; this task is to prove them under real
concurrency and wire the test harness + orchestrator to them. If you change the
isolation model, update the isolation matrix in the canon doc.
