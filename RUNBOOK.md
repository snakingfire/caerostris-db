# RUNBOOK — launching & supervising the caerostris-db autonomous build

This repo is scaffolded so a **single command** starts a swarm of Claude agents
that designs, builds, tests, and self-grades a graph database — autonomously, in
parallel, for ~4 hours — then hands you a testable engine.

This is the human's guide. The agents' guide is
[`docs/commanders-intent.md`](docs/commanders-intent.md).

## What you're launching

| Piece | Where | Role |
|------|-------|------|
| **Commander's intent** | `docs/commanders-intent.md` | The north star every agent obeys. |
| **Master rubric** | `docs/requirements/master-rubric.md` | The single graded source of truth (weighted, gated). |
| **Operating model** | `docs/process/autonomous-operating-model.md` | Roles, cadence, agile-parallel doctrine, pace. |
| **Board** | `.project/board/tasks/` | File-per-item work tracker (`scripts/board/ls.sh`). |
| **Mainspring** | `.claude/workflows/mainspring.js` | The orchestration loop (one bounded epoch per run). |
| **Agents** | `.claude/agents/` | Steering committee + worker/reviewer definitions. |
| **Crons** | created by `/launch` | grader (20 min) + pace-marshal (10 min, relaunches mainspring). |
| **Reports** | `.project/reports/` | Rubric grade + progress, committed every 20 min. |
| **Pace ledger** | `.project/pace/deadline.md` | T0 + deadline markers + checkpoint targets. |

## How to launch

1. Open a **fresh Claude Code session** in this repo (Opus-tier recommended; the
   swarm dispatches its own per-task models).
2. Make sure the environment is ready:
   - Toolchain available (`direnv allow`, or rustup via `rust-toolchain.toml`).
   - A local S3 mock for integration tests (MinIO or moto/localstack) — see
     [`docs/process/testing-and-benchmarks.md`](docs/process/testing-and-benchmarks.md).
     Real AWS S3 / EC2 are optional and wired later when you supply credentials.
3. Run the launch command:

   ```
   /launch
   ```

   It stamps **T0**, fills the deadline markers, arms the two crons, and kicks off
   the first **mainspring** epoch. Then it hands off and the run is autonomous.

That's it. From here the swarm runs itself until **T0+5:00** (hard stop) — you
return at **T0+4:00** to test.

## What happens during the run

- **First epoch:** the steering committee adversarially ratifies the intent +
  rubric (filing any feasibility objections as P0 board items), the planner
  decomposes all 10 epics into ready tasks, and the first wave of work starts.
- **Design-before-code is enforced:** the latency selectivity-envelope model
  (`SPIKE-0001`), the S3 commit protocol + TLA+ model (`SPIKE-0002`), and the
  storage format spec (`SPIKE-0003`) must be steering-ratified before their
  dependent implementation tasks become workable. This is the "formally provable
  before any line of code" requirement, operationalized.
- **Steady state:** many agents in parallel — specifying, researching,
  implementing (TDD in isolated worktrees), reviewing (adversarial + pre-mortem),
  landing (single-writer onto `main`), testing, proving, sourcing datasets,
  grading, and grooming the board. See the operating model.
- **Every 20 min:** a committed rubric report in `.project/reports/`.
- **Every ~hour:** an hourly release + notes (`docs/process/release-hourlies.md`).

## How to supervise (optional — it's autonomous)

- **Live progress:** `/workflows` shows the mainspring tree; `git log --oneline`
  shows landings/reports rolling in.
- **Score:** read the newest file in `.project/reports/`.
- **Backlog:** `scripts/board/ls.sh` (or `scripts/board/ls.sh ready P0`).
- **Pace:** `.project/pace/deadline.md` — expected-vs-actual per marker.

## Controls

- **Stop early:** create `.project/STOP` (a gitignored run-local sentinel) — the
  pace-marshal stops relaunching epochs and the next orient halts. Or just tell
  the session to stop.
- **Course-correct:** drop a P0 task on the board (`scripts/board/new-task.sh`)
  or edit `docs/commanders-intent.md`; agents read the latest each epoch.
- **Give it AWS:** put credentials in the environment (never in the repo — see
  [`docs/process/open-source-guardrails.md`](docs/process/open-source-guardrails.md)),
  and file a task to flip E2E/perf from the mock to real S3 + EC2.

## At T0+4:00 — your testing window

Expect (per the rubric's definition of done): every GATE category ≥ 90, overall
≥ 90, openCypher TCK at 100%. To verify:

```bash
cargo test --all-features            # unit + integration (uses the S3 mock)
cargo llvm-cov --summary-only        # coverage (target ≥90%)
cargo bench                          # criterion benches incl. the headline query
# TCK pass-rate + formal-model check: see the latest report and docs/process/
```

Then read the final `.project/reports/` grade. Gaps, if any, are named honestly on
the board and in the report — not hidden.

## If something's wrong

- **Run stalled / no new commits:** check `/workflows`; the pace-marshal cron
  relaunches mainspring every ~10 min — confirm the cron exists and `.project/STOP`
  is absent.
- **Red CI / failing hourly:** that's a P0 by policy; the swarm should self-file
  it. If not, drop a `BUG-` item on the board.
- **It's drifting from intent:** the latency theorem and the GATE categories are
  the guardrails — edit `docs/commanders-intent.md`/the rubric and the next epoch
  realigns.
