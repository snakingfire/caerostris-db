# Simulated PR Workflow — caerostris-db

> The fast local protocol for landing parallel work without agents stepping on
> each other. Read this alongside
> [`autonomous-operating-model.md`](autonomous-operating-model.md) (work
> lifecycle) and [`task-board-protocol.md`](task-board-protocol.md) (board
> hygiene). Every code change to `main` flows through this protocol — no
> exceptions.

## Why worktrees, not branches on a shared checkout

Many agents run concurrently and several will have modified files at the same
time. A shared working tree means constant conflicts and corrupt state. The
harness supports `isolation: 'worktree'` for agents that mutate files — each
worker gets its own git worktree, its own build cache, and its own CI run.
Landing is serialized by a single integrator who owns `main`. This mirrors the
database's own single-writer / multi-reader architecture: readers (reviewers)
work concurrently against immutable snapshots; the writer (integrator) is the
only one who advances the canonical state.

## Branch naming

```
work/<ID>-<slug>
```

Examples: `work/T-0142-btree-text-index`, `work/SPIKE-0003-latency-envelope`.

The `<ID>` matches the board item exactly. `<slug>` is a short kebab summary.
No other prefixes. No personal namespaces. The integrator relies on the `work/`
prefix to identify mergeable branches.

## Opening a PR (the worker's steps)

```bash
scripts/pr/open.sh <ID> <slug>
```

`open.sh` does three things:

1. Creates a git worktree at `.worktrees/<ID>-<slug>` and checks out a new
   branch `work/<ID>-<slug>` from the current tip of `main`.
2. Writes a PR description stub at `.worktrees/<ID>-<slug>/PR.md` (see template
   below).
3. Prints the worktree path so the worker can `cd` into it and begin.

The worker then implements the task TDD-first inside that worktree, commits
frequently (small, logically coherent commits), and fills in `PR.md` before
requesting review.

## PR description template

Every `PR.md` must contain all sections. Partial descriptions are not reviewed.

```markdown
## Board item
<!-- Link to the board item file, e.g. .project/board/tasks/T-0142-btree-text-index.md -->

## Rubric refs
<!-- Cat numbers from master-rubric.md this advances, e.g. Cat 5, Cat 3 -->

## Acceptance criteria (from board item)
- [ ] ...
- [ ] ...

## Summary of change
<!-- What changed and why — 3–8 sentences. Reference the design/ADR if one exists. -->

## Test evidence
<!-- Paste or link the output of: cargo nextest run, cargo llvm-cov, ./format_code.sh -->
<!-- At minimum: test count, coverage %, any benchmark numbers relevant to the change. -->

## Review gate
- [ ] adversarial-reviewer sign-off (see [adversarial-review-loops.md](adversarial-review-loops.md))
- [ ] premortem-analyst sign-off (see [adversarial-review-loops.md](adversarial-review-loops.md))
- [ ] `./format_code.sh` green
- [ ] `cargo nextest run` green
- [ ] coverage not regressed
- [ ] board item updated to `in_review`
```

Leave the review-gate checkboxes unchecked. The adversarial-reviewer and
premortem-analyst fill them in by appending their verdict records (see
[`adversarial-review-loops.md`](adversarial-review-loops.md)).

## Keeping diffs small

- Commit in logical slices as you go. A PR that touches 500+ lines is a red
  flag; split it into sequential tasks if possible.
- Pull `main` into your worktree branch frequently (`git fetch origin main &&
  git rebase origin/main`) so conflicts surface early and stay small.
- Never rewrite history after opening for review (no `--amend`, no `rebase -i`
  once reviewers have seen the branch). Add a follow-up commit instead.

## The review gate

Before landing, the PR must clear **both** quality loops defined in
[`adversarial-review-loops.md`](adversarial-review-loops.md):

1. **Adversarial code review** — an `adversarial-reviewer` agent tries to
   break the diff (correctness, security, simplicity). Verdict appended to
   `PR.md`. Must be `approve` or `changes_requested` (the author addresses and
   re-requests).
2. **Pre-mortem** — a `premortem-analyst` agent assumes the change shipped and
   caused an incident; enumerates failure modes; gates on mitigations committed.

Both sign-offs must be `approve` in `PR.md` before the integrator is called.
`./format_code.sh` and `cargo nextest run` must be green in the worktree at
the tip commit.

## Landing (the integrator's steps)

```bash
scripts/pr/land.sh <ID>-<slug>
```

`land.sh` does the following in order:

1. Reads `PR.md` — asserts both review-gate checkboxes are signed off.
2. Runs `./format_code.sh` inside the worktree — aborts on any failure.
3. Runs `cargo nextest run` — aborts on any failure.
4. Rebases the branch onto current `main` (fast-forward if possible; rebase if
   diverged).
5. Merges to `main` with `--no-ff` to preserve the branch topology.
6. Pushes `main`.
7. Removes the worktree and deletes the branch.
8. Updates the board item to `done` (appends landing commit hash to the Notes
   log).

The integrator is **the only agent that writes to `main`**. It serializes all
merges. If two PRs are simultaneously ready, the integrator lands them one at a
time in rubric-weight order (higher-weight gates first). This is not a
bottleneck — the integrator is fast; workers are the long pole.

## Conflict handling

- **Worker-level (pre-review):** rebase onto `main` frequently. Small tasks
  rebase cleanly; large ones accumulate debt. Split `L` tasks before starting.
- **At landing time:** if `land.sh` encounters a rebase conflict, it aborts and
  returns the branch to the worker with a conflict report. The worker resolves,
  re-runs `./format_code.sh` + tests, and re-requests review (the review-gate
  checkboxes reset to unchecked).
- **Structural conflicts** (two branches touching the same interface in
  incompatible ways) are escalated to the relevant steering-committee member
  before either branch lands. The steering member adjudicates; the loser
  rebases against the winner.

## Summary: the single-writer invariant

Concurrent workers in isolated worktrees — many parallel readers of `main` who
write only to their own branch. One integrator who serializes writes to `main`.
The board records every landing commit. The invariant holds as long as nobody
pushes to `main` directly.
