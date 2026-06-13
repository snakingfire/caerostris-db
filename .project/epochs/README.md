# Epoch hand-off artifacts

> The serialised in-flight context an epoch hands to its successor when the
> mainspring recycles near the per-run agent cap. Produced by
> [`scripts/board/epoch-handoff.sh`](../../scripts/board/epoch-handoff.sh); read
> by the relaunched epoch. The full procedure lives in
> [`docs/process/epoch-recycling.md`](../../docs/process/epoch-recycling.md).

## Why this exists

The swarm runs under a per-run agent cap (~16 concurrent agents, bounded
rounds). When a run approaches the cap, throughput would otherwise collapse to
zero until a human cold-restarts it. Epoch recycling avoids that: the outgoing
epoch writes a hand-off artifact, a fresh epoch launches, reads it, and resumes
at full throughput **without re-executing completed work or losing in-progress
state**. (Board item `T-0004`, rubric Cat. 12; `EPIC-010`.)

## File naming

```
.project/epochs/epoch-<N>.md
```

`<N>` is the epoch number (monotonic, 1-based). `epoch-handoff.sh N` writes a
specific number; with no argument it uses the next number after the highest
existing artifact. Markdown (not JSON/binary) is deliberate — any agent or human
can read it without tooling, per the task's design note.

## Schema (sections every artifact carries)

| Section | Content | Source of truth |
|---------|---------|-----------------|
| Header table | `epoch`, `timestamp` (ISO-8601 UTC `Z`), `open items` count, `blockers` count, `rubric report` name, `rubric overall` score | board + latest rubric report |
| **Open task IDs** | every board item still resumable — `ready`, `in_progress`, `in_review`, `blocked` — with `status` + `title`. `done` / `dropped` are **excluded** so the next epoch never re-runs finished work. | `.project/board/tasks/*.md` frontmatter `status` |
| **Blockers** | items in `blocked` state, called out separately (resolve / reland first) | frontmatter `status: blocked` |
| **Resume checklist** | the ordered steps the next epoch follows (checkpoint → unblock → claim) | `docs/process/epoch-recycling.md` |

### The artifact is a *summary*, not the source of truth

The committed board (`.project/board/tasks/*.md` `status` fields) is always the
authority on what is done vs. open. The hand-off artifact is a fast, readable
snapshot so the relaunch can orient in seconds — but the relaunch re-derives the
claimable set from the live board (via `scripts/board/claim.sh`), so a stale or
missing artifact degrades gracefully (slower orient, never wrong work).

## Lifecycle

1. Outgoing epoch (or the pace-marshal) runs `scripts/board/checkpoint.sh` — it
   must exit 0 (clean, resumable tree) before standing down.
2. Outgoing epoch runs `scripts/board/epoch-handoff.sh <N+1>` and commits the
   artifact (`board:`-prefixed, like all board commits).
3. A fresh epoch launches, reads `epoch-<N+1>.md`, and resumes (see the resume
   checklist in the artifact and the procedure doc).

Old artifacts are retained as an audit trail of how the run progressed; they are
small and human-readable.
