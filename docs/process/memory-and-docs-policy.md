# Memory and Docs Policy — caerostris-db

> **Rubric anchor:** Cat. 12 (Engineering & process health). A stale CLAUDE.md,
> missing ADRs, or undocumented decisions each cost Cat. 12 score. The grader
> checks for doc currency. See [`../requirements/master-rubric.md`](../requirements/master-rubric.md).

---

## The rule in one sentence

**When you learn something non-obvious that would help the next agent, write it
down. When a doc becomes false, fix it in the same change that made it false.**

This is not aspirational — it is a definition-of-done requirement. A task is not
done until the docs, ADRs, and agent memory that the task touched are current.

---

## Roles

### docs-memory-curator

A dedicated swarm role (see
[`autonomous-operating-model.md`](autonomous-operating-model.md)) that runs
continuously alongside the build-test-land pipeline:

- Keeps the root `CLAUDE.md` accurate: architecture-as-built, where things live,
  current conventions, current rubric score, current phase.
- Prunes stale claims from `CLAUDE.md` and other process docs when the underlying
  implementation changes.
- Ensures new ADRs are filed when major decisions land.
- Reads `.project/knowledge/` and `docs/notes/` regularly; promotes recurring
  questions from the knowledge base into `CLAUDE.md` when they rise to the level
  of project-wide convention.

Any agent may (and should) directly update docs as part of their task. The curator
is the backstop, not the sole owner.

---

## Document types and their locations

| Document type | Location | Owner | Update trigger |
|---|---|---|---|
| Root agent guide (architecture-as-built) | `CLAUDE.md` | docs-memory-curator | Any change to architecture, conventions, or where things live |
| Architecture Decision Records | `docs/adr/NNNN-*.md` | any agent (curator reviews) | Every major architectural or design decision |
| Design specs | `docs/superpowers/specs/` | specifying agent | When a spec is drafted or revised |
| Decision log entries | `.project/decisions/NNNN-*.md` | deciding agent | Every autonomous decision (design-level or reversible-but-notable) |
| Agent knowledge base | `.project/knowledge/` | any agent | When a non-obvious lesson is learned |
| Short notes / gotchas | `docs/notes/` | any agent | When a sharp edge is discovered |
| Rubric grade reports | `.project/reports/` | rubric-grader | Every 20-minute grading cycle |
| Benchmark history | `.project/reports/benchmark-history.jsonl` | perf-engineer | Every benchmark run |
| Board items | `.project/board/` | planner, workers | Status changes as work progresses |

---

## ADRs (Architecture Decision Records)

An ADR is required for every decision that:

- Establishes or changes the storage format, commit protocol, or query execution model.
- Adds, removes, or changes a major dependency (crate or external tool).
- Changes the public API surface (engine API, Python bindings, CLI).
- Chooses between competing technical approaches after a spike.
- Establishes a project-wide convention.

ADRs are **not** required for implementation details within a task, for obvious
choices with no real alternatives, or for decisions that only affect one file.

### ADR format

```
docs/adr/NNNN-short-title.md
```

```markdown
# ADR-NNNN: Short Title

**Status:** accepted | superseded by ADR-MMMM | deprecated
**Date:** YYYY-MM-DD
**Deciders:** (agent roles involved)

## Context

What situation prompted this decision?

## Decision

What was decided?

## Consequences

What becomes easier? What becomes harder? What must now be maintained?

## Alternatives considered

What was rejected and why?
```

When an ADR is superseded, update its `Status` line and link to the new ADR. Do
not delete old ADRs.

---

## Design specs

Specs live in `docs/superpowers/specs/` and are the formal artifact that steering
ratifies before implementation begins (see
[`adversarial-review-loops.md`](adversarial-review-loops.md)). A spec is not a
work-in-progress log — it is the agreed design. Keep specs updated when the design
evolves; flag drift between spec and implementation as a bug.

---

## Decision log

Every autonomous decision (not just design-level ones) gets a log entry in
`.project/decisions/NNNN-*.md`. The format is brief:

```markdown
# Decision NNNN: Title

**Date:** YYYY-MM-DD
**Agent:** (role)
**Reversible:** yes/no

## Decision

One sentence.

## Rationale

Why this, not the alternatives?

## Alternatives rejected

- Option B: rejected because …
```

Decision log entries are append-only. Never delete or rewrite a decision entry;
if a decision is reversed, write a new entry referencing the original.

---

## Agent knowledge base (`.project/knowledge/`)

This is the **persistent short-term memory** of the swarm — the place where
lessons learned propagate across agents without requiring re-discovery.

### Structure

```
.project/knowledge/
  storage-format.md       # gotchas about the storage layout
  s3-mock-quirks.md       # MinIO/moto-specific behaviours that differ from real S3
  proptest-tips.md        # how to write effective property tests for this codebase
  opencypher-edge-cases.md # TCK scenarios that are surprisingly tricky
  performance-notes.md    # non-obvious performance findings
  ...
```

One file per topic. Files grow by appending; do not restructure a knowledge file
unless you have strong reason (it disrupts the history).

### Protocol

**Before starting a task:** read the knowledge files that are relevant to your
work area. Knowledge files are short; reading them takes seconds and prevents
repeating mistakes.

**After completing a task:** if you encountered anything non-obvious — a gotcha, a
wrong turn, a surprising interaction, a "don't do X because Y" — write a brief
note. One or two sentences is enough. Include the date and your role.

Example entry:

```markdown
## 2026-06-13 — implementer

MinIO in path-style mode requires `CAEROSTRIS_S3_FORCE_PATH_STYLE=true` or
object_store will generate virtual-hosted-style URLs that resolve to 404.
Wasted ~30 min debugging this. See TASK-047.
```

### `docs/notes/`

For notes that are codebase-facing (visible to anyone reading the repo, not just
swarm agents), use `docs/notes/`. These might be referenced from code comments or
from external docs. Format is the same as `.project/knowledge/`.

---

## CLAUDE.md maintenance

`CLAUDE.md` is the **first file a new agent reads**. Its accuracy is therefore a
force multiplier for the whole swarm.

### What CLAUDE.md must always reflect

- The current architecture (storage layer, query engine, attach modes, bindings).
- Where things live in the codebase (key files, module layout).
- Current conventions (naming, error handling, async model, test patterns).
- Current rubric score (updated after each grading cycle).
- Current phase (what's built, what's in progress, what's not yet started).
- The dev workflow (build, test, format commands).

### What CLAUDE.md must not contain

- Aspirational future plans (those belong in specs or the board).
- Stale architecture descriptions that don't match the code.
- Step-by-step tutorials (those belong in `docs/`).

### Update protocol

- **Any agent** that makes a change that affects any of the above must update
  `CLAUDE.md` in the same commit (or the same PR) as the change.
- The **docs-memory-curator** does a full CLAUDE.md review pass every ~20 minutes
  and files a gap task for any stale claim found.
- If you are unsure whether a change warrants a CLAUDE.md update, ask: "Would a
  new agent reading this doc get a misleading picture of the codebase?" If yes,
  update it.

---

## The two-line test

Before closing any task, ask:

1. **Did I learn anything non-obvious?** If yes: is it in `.project/knowledge/`?
2. **Did I make anything currently documented become false?** If yes: is the doc
   fixed?

If both answers are "no" or "yes and done," the task is complete. If either is
"yes and not done," finish the docs first.
