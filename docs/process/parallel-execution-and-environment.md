# Parallel Execution & Self-Provisioned Environment

> The swarm runs **many agents at once**. This document is the contract that lets
> them work in parallel **without conflicting** — and that makes the run set up
> its **own** environment with **no human prerequisite**. There is no "make sure
> the mock is running" step for a human: the swarm provisions and heals it itself.
>
> If you are about to run a test, ingest data, build, or land code, the relevant
> isolation rule below is **mandatory**, not advisory.

## Principle: one shared environment, isolated namespaces

A single local S3-compatible mock serves the **whole run** (starting one server
once avoids port wars). Every agent **isolates its own data inside that shared
server** by using a unique bucket/prefix. Source: each agent works in its own git
worktree, so source files never collide. Landing: a single integrator serializes
merges to `main`. The result: thousands of parallel operations, zero conflicts.

## Self-provisioning (no human step)

The environment is brought up by the swarm, idempotently and safely under
concurrency:

- **`scripts/env/up.sh`** — idempotently starts the shared mock and writes the
  shared `local.env`. Safe to call concurrently (atomic lock); a no-op if already
  healthy. Provision ladder: **Docker MinIO → `moto_server` → `pip install
  moto[server]` → (last resort) in-process memory backend** (unit tests only;
  integration needs a server, so the provisioning task escalates to install one).
- **`scripts/env/bucket.sh <ID>`** — prints/creates an **isolated** bucket+prefix
  for a work item. Use `eval "$(scripts/env/bucket.sh T-0042)"` to get
  `CAEROSTRIS_S3_BUCKET` + `CAEROSTRIS_S3_PREFIX`.
- **`scripts/env/down.sh`** — tears the mock down (end of run / cleanup).

**When provisioning happens, automatically:**
1. The **first mainspring epoch** ensures the env is up before dispatching work.
2. The **pace-marshal cron** re-checks env health every tick and re-provisions if
   the mock died.
3. **Any agent** may call `scripts/env/up.sh` before integration tests — it is
   idempotent, so this is the self-heal path. **Always do this rather than
   assuming the mock is up.**

### Shared run-state lives in the MAIN repo (critical for worktrees)

Agents run in worktrees under `.worktrees/<ID>`, but the mock is ONE server for
the whole run. So run-state (`local.env`, locks, pidfiles) lives in the **main
worktree**, resolved via the git *common* dir — not the per-worktree copy:

```bash
GIT_COMMON=$(git rev-parse --path-format=absolute --git-common-dir)  # /abs/main/.git
ENV_DIR="$(dirname "$GIT_COMMON")/.project/env"                       # shared, gitignored
source "$ENV_DIR/local.env"
```

Every worktree thus reads the **same endpoint and bucket base**. `.project/env/`
is gitignored — it is run-local state, never committed (and contains the mock's
public default creds, never real secrets).

## The isolation matrix (how each shared resource is de-conflicted)

| Resource | Conflict if shared naively | Isolation rule |
|----------|----------------------------|----------------|
| **Source tree** | two agents editing the same files | **One git worktree per work item** (`scripts/pr/open.sh <ID>` → `.worktrees/<ID>` on branch `work/<ID>-…`). See [`simulated-pr-workflow.md`](simulated-pr-workflow.md). |
| **S3 object store** | tests stomping each other's objects / assuming a clean bucket | **One shared mock**, per-item **bucket+prefix** from `scripts/env/bucket.sh <ID>`. Tests **create + tear down their own namespace** and never assume global cleanliness. |
| **Network ports** | each agent starting its own mock → port clash | **Don't start your own.** Use the shared endpoint from `local.env`. `up.sh` picks one free port once; everyone reuses it. |
| **`main` branch** | concurrent merges racing | **Single-writer landing.** Only the `integrator` runs `scripts/pr/land.sh`; merges are serialized (mirrors the DB's own single-writer model). Workers never push to `main`. |
| **The board** | two agents claiming/editing the same item | **One file per item** + the claim protocol (set `assignee`+`status`, commit `board:`, re-read before starting). See [`task-board-protocol.md`](task-board-protocol.md). |
| **Cargo build** | shared `target/` lock contention | Each worktree builds in **its own `target/`** (the default). Do **not** set a shared `CARGO_TARGET_DIR` across worktrees. Cargo's own registry lock briefly serializes deps — that's fine. |
| **Mainspring epochs** | two orchestrator runs dispatching the same tasks | The pace-marshal relaunches an epoch **only if none is running** (checks active workflows first). Never launch `mainspring` while one is active. |
| **Hourly releases** | two agents tagging at once | Release cutting is **single-owner** + lock-guarded (`scripts/release-hourly.sh` refuses if a release is in progress). See [`release-hourlies.md`](release-hourlies.md). |
| **Datasets** | concurrent downloads of the same large file | Shared gitignored `/data/`; download under a lock + checksum; dedupe (don't re-fetch what's present). See [`datasets.md`](datasets.md). |
| **Run sentinels** | stale `.bootstrapped`/`STOP`/lock | Run-local, gitignored, in the **main** `.project/`. Treated as filesystem signals, never committed. |

## Test authoring rules (so integration tests are parallel-safe)

1. **Ensure env, don't assume it:** at test setup, run `scripts/env/up.sh` (idempotent), then source `$ENV_DIR/local.env`.
2. **Get your own namespace:** `eval "$(scripts/env/bucket.sh <ID-or-test-name>)"` → unique `CAEROSTRIS_S3_BUCKET`/`CAEROSTRIS_S3_PREFIX`.
3. **Never hardcode a bucket** or assume it's empty. Create what you need under your prefix; clean up after (or rely on the unique prefix to avoid cross-talk).
4. **Unit tests** that don't need a server use the in-process `object_store` memory backend — no env needed, fully parallel.
5. **The same code path** (the `object_store` abstraction) runs against the mock now and real S3 later — only the endpoint/creds change, supplied by env. See [`testing-and-benchmarks.md`](testing-and-benchmarks.md).

## Definition of "environment is handled"

A fresh machine with **only** the repo + a Rust toolchain (and Docker *or* Python
available) can run `/launch` and the swarm stands up everything it needs itself,
parallel-safe, with no human touching the mock. If neither Docker nor Python is
present, the provisioning task installs/obtains a server or escalates a P0 — it
does **not** silently fall back and pretend integration coverage exists.
