---
description: Launch the caerostris-db autonomous build swarm (stamps T0, starts the grader + pace-marshal crons, and kicks off the mainspring orchestrator).
---

# /launch — start the autonomous build run

You are the **launch conductor**. Your job is to set the autonomous swarm in
motion and then get out of the way. Execute these steps **in order**, then stop.

> Do not start designing or implementing the database yourself. The swarm does
> that. You only arm the machinery.

## 0. Pre-flight (read + verify)

1. Read `docs/commanders-intent.md`, `docs/requirements/master-rubric.md`, and
   `docs/process/autonomous-operating-model.md` so you understand the run.
2. Confirm the working tree is clean and on `main` (`git status`). If dirty,
   commit the scaffold first (`chore: …`).
3. Confirm there is **no** `.project/STOP` file (remove a stale one only if you
   are certain a previous run ended). Confirm the agent definitions exist
   (`ls .claude/agents/`) and `scripts/` are executable (`chmod +x` if not).

## 1. Stamp the clock (T0)

Get the current wallclock: `date -u +"%Y-%m-%dT%H:%M:%SZ"` and the local time.
Edit `.project/pace/deadline.md`:

- Set **T0** to now.
- Compute and fill **T0+4:00** (feature-complete / rubric-green target) and
  **T0+5:00** (hard deadline) as absolute timestamps.
- Fill the wallclock column of the checkpoint table (each marker = T0 + offset).

Commit: `chore(pace): stamp T0 and deadline markers for the autonomous run`.

## 2. Arm the crons (use ToolSearch to load `CronCreate`)

Create **two** recurring jobs. Each cron prompt must be **self-contained** (it
runs unattended in a fresh context), so each prompt should start by telling the
agent to read the canon docs + its agent definition.

**Cron A — `rubric-grader`, every 20 minutes (`*/20 * * * *`):**
> You are the rubric-grader for caerostris-db. Read docs/requirements/master-rubric.md,
> .claude/agents/rubric-grader.md, and .project/pace/deadline.md. Grade the project
> against EVERY rubric category with EVIDENCE (cite artifacts: passing tests,
> benchmark numbers, committed proofs, the live TCK pass-rate, cargo-llvm-cov
> coverage). Fill the scoreboard table, compute the weighted overall, write a
> timestamped report to .project/reports/, commit it (`report: rubric grade T+…`),
> and file gap-closing tasks to the board for any category below its checkpoint
> target. Note expected-vs-actual vs the pace ledger. Do not implement features.

**Cron B — `pace-marshal`, every 10 minutes (`*/10 * * * *`):**
> You are the pace-marshal for caerostris-db. Read .claude/agents/pace-marshal.md,
> .project/pace/deadline.md, docs/process/autonomous-operating-model.md. (1) Groom
> the board: unblock items whose deps are done, re-prioritize toward the lowest
> GATE rubric categories if behind pace, split stuck L tasks, nudge items stuck
> in_review. (1b) Keep the environment healthy: run `scripts/env/up.sh`
> (idempotent) — it re-provisions the shared local S3 mock if it died; file a P0
> if provisioning fails (see docs/process/parallel-execution-and-environment.md).
> (2) If the current wallclock is past T0+5:00 (hard deadline), create the
> `.project/STOP` sentinel (gitignored, run-local) and stop relaunching.
> (3) Otherwise, check active
> tasks/workflows (TaskList): if **no** `mainspring` workflow is currently running
> and `.project/STOP` is absent, relaunch one epoch with
> `Workflow({ name: "mainspring" })`. Keep the engine running. Raise a P0 board
> item if any GATE category is sliding.

Record the created cron ids in `.project/pace/deadline.md` log.

## 3. Kick off epoch 1

Start the orchestrator immediately (don't wait for the first cron tick):

```
Workflow({ name: "mainspring" })
```

This first epoch will: orient → have the steering committee ratify intent+rubric
→ have the planner decompose the whole board → run the first wave of
design/research + implement→review→pre-mortem→land work.

## 4. Hand off

Print a short status: T0 + the two deadline markers, the cron ids, and the
mainspring run id. Tell the human:

- Progress reports land in `.project/reports/` every 20 minutes (committed).
- The board lives in `.project/board/tasks/` (`scripts/board/ls.sh`).
- Hourly builds + notes appear per `docs/process/release-hourlies.md`.
- The run stops itself at T0+5:00 (the `.project/STOP` sentinel); they can stop
  early by creating `.project/STOP` (or via the pace-marshal).

Then **stop**. The swarm is autonomous from here until T0+4:00, when the human
returns to test against the rubric.
