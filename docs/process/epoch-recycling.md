# Epoch Recycling, the Board/Pace Dashboard & STOP-Sentinel Handling

> **Rubric anchor:** Cat. 12 (Engineering & process health) and `EPIC-010`
> (harden the autonomous harness). Board item `T-0004`.
> See [`../requirements/master-rubric.md`](../requirements/master-rubric.md) and
> [`autonomous-operating-model.md`](autonomous-operating-model.md) (§ Self-improvement).

---

## Purpose

The swarm runs under a **per-run agent cap** (the harness bounds concurrent
agents and rounds per workflow invocation — see
[`../../.claude/workflows/mainspring.js`](../../.claude/workflows/mainspring.js),
`MAX_ROUNDS` / `MIN_BUDGET`). Without recycling, throughput collapses to zero
each time a run hits the cap, and the deadline budget bleeds out waiting for a
manual cold restart. **Epoch recycling** keeps the build moving for the full
4-hour window: an outgoing epoch serialises its in-flight context, a fresh epoch
launches, reads that context, and resumes at full throughput — **without
re-executing completed work or losing in-progress state.**

This document covers the three pieces of harness machinery that make a recycle
clean and a stand-down safe:

1. **Epoch hand-off artifacts** — what the outgoing epoch writes.
2. **The board/pace dashboard** — the at-a-glance state the grader/pace-marshal
   regenerate each cycle.
3. **STOP-sentinel handling** — how an epoch reaches a *resumable* checkpoint
   before standing down.

---

## 1. Epoch hand-off artifacts

### What & where

`scripts/board/epoch-handoff.sh [N]` writes a lightweight, human-readable
markdown artifact at:

```
.project/epochs/epoch-<N>.md
```

Markdown over JSON/binary is deliberate: any agent (or human) can inspect it
without special tooling. The schema is documented in
[`../../.project/epochs/README.md`](../../.project/epochs/README.md). It captures:

- a **timestamp** (ISO-8601 UTC) and the **epoch number**;
- the **open task IDs** — every board item still resumable (`ready`,
  `in_progress`, `in_review`, `blocked`). `done` / `dropped` are **excluded** so
  the next epoch never re-runs finished work;
- the current **blockers** (items in `blocked` state);
- the latest **rubric snapshot** (overall score + report name).

The artifact is **read-only on the board and on git** — its only write is the one
artifact file — so it is safe to produce at a stand-down checkpoint.

### Source of truth vs. summary

The committed board (`.project/board/tasks/*.md` `status` fields) is **always**
the authority on done-vs-open. The hand-off artifact is a fast summary so a
relaunch can orient in seconds. The relaunch re-derives the claimable set from
the live board (`scripts/board/claim.sh`), so a stale or missing artifact
degrades gracefully — slower orient, never wrong work.

---

## 2. The board/pace dashboard

`scripts/board/dashboard.sh` regenerates a read-only snapshot at
`.project/reports/dashboard-<UTC-timestamp>.md` containing:

- **(a)** item counts by status (`backlog`/`ready`/`in_progress`/`in_review`/
  `blocked`/`done`) and by epic;
- **(b)** a **pace metric** — items completed / elapsed time since T0, with a
  projected drain of the remaining board at the current rate;
- **(c)** the **latest rubric** overall score (parsed from
  `.project/reports/rubric-*.md`, tolerant of the grader's `**~NN**` estimate
  format);
- **(d)** the current **blockers**.

It uses only coreutils + awk, makes a single pass over the board frontmatter (no
per-field subprocess fan-out), is **read-only on the board**, and runs in well
under a second on the live board (acceptance budget: < 5 s). Run it as often as
you like — repeatedly, idempotently, with no side effects beyond the one file.

**Cadence:** the `rubric-grader` (every 20 min) and the `pace-marshal` (every
~10 min) call `scripts/board/dashboard.sh` and commit the resulting dashboard
alongside their report (`board:`-prefixed). CI exposes the script as a callable
step (`.github/workflows/ci.yml` → *board-dashboard*) so it cannot silently rot.

---

## 3. STOP-sentinel handling & the clean checkpoint

### The sentinel

A STOP request is the file `.project/STOP`. The mainspring loop checks for it
every orient and, when present, **releases its claims and stands down** (see
`mainspring.js`: `state.stop` → release `claimed_ids`, return `stopped`). The
`pace-marshal` writes the sentinel to halt the run (e.g. at the hard deadline).

### The clean-checkpoint gate

Before an epoch stands down — whether because of a STOP, a budget floor, or a
recycle — it must leave a **resumable** tree. `scripts/board/checkpoint.sh`
verifies exactly that and **exits non-zero if it is not**:

1. **Git is clean** — no uncommitted/staged/partial changes. A half-written file
   or an un-committed board edit is unrecoverable context, so it fails the gate.
2. **Every `in_progress` item is noted** — each board item still
   `status: in_progress` must carry at least one bullet in its `## Notes / log`
   section, so the relaunch knows what was in flight and where to pick up.
3. It **reports** whether `.project/STOP` is present (final stand-down) or absent
   (still running).

A relaunch or the pace-marshal gates on the exit code.

---

## The relaunch procedure (step by step)

The next epoch resumes **without duplicate work** as follows:

1. **Verify the checkpoint.** Run `scripts/board/checkpoint.sh`. It must exit 0
   (clean, resumable). If it fails, fix what it names (commit dirty state; add a
   handoff note to any un-noted `in_progress` item) before relaunching.
2. **Read the latest hand-off.** Open the highest-numbered
   `.project/epochs/epoch-<N>.md` for the open set, blockers, and rubric
   snapshot. This is your orientation summary — not your work list.
3. **Re-open the cascade.** Run `scripts/board/unblock.sh` to flip any backlog
   item whose deps are now `done` to `ready` (it commits the change), so
   newly-unblocked work is claimable this round.
4. **Claim from the live board.** Run `scripts/board/claim.sh claim <lane> <max>`.
   Because claiming reads the **committed board status**, anything already `done`
   is simply not claimable — no re-execution. The hand-off's "open task IDs"
   section should match the claimable set; if it doesn't, trust the board.
5. **Resume the loop.** Implement → review → pre-mortem → land per item, exactly
   as a first epoch does. The first round's `done` work continues uninterrupted.
6. **Recycle again** when the budget floor (`MIN_BUDGET`) approaches: write the
   next hand-off (`scripts/board/epoch-handoff.sh`), checkpoint, and relaunch.

### Why this never re-executes completed work

- `done` items are **excluded** from the hand-off's open set.
- `claim.sh` only ever claims `ready` / `in_review` / `blocked` items — a `done`
  item is invisible to it.
- The board `status` is committed to git, so a relaunched epoch sees the same
  authoritative state regardless of the artifact.

### Sustaining throughput across 4h

The recycle is cheap (write one markdown file, relaunch), so the pace-marshal can
trigger it well before the cap bites — the target is **≥ 1 board item reaching
`done` per hour** with **no idle gap > 15 min** due to cap exhaustion (the
acceptance definition in `T-0004`). The dashboard's pace metric is the live
read-out the pace-marshal uses to decide when to recycle.
