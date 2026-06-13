# Architecture Decision Records (ADRs)

This directory contains Architecture Decision Records for caerostris-db. An ADR
captures a significant design decision: the context that motivated it, the
decision itself, the alternatives that were considered, and the consequences
(positive and negative) of the choice.

## What belongs here

Every **major or irreversible design decision** must have an ADR before any
dependent implementation task is marked `ready`. In particular:

- Storage format and object layout (Cat. 2 in the master rubric)
- Commit and concurrency protocol (ACID / snapshot isolation — Cat. 1, Cat. 7)
- Latency envelope definition and proof strategy (Cat. 3)
- openCypher semantic choices and TCK phasing (Cat. 4)
- Index interface / pluggability contract (Cat. 5)
- Python binding surface and packaging approach (Cat. 8)
- Any formal-model or TLA+ scope decisions (Cat. 11)
- Cross-cutting process or tooling changes that bind future work

Reversible, local, or purely implementation-level decisions are recorded in
`.project/decisions/NNNN-*.md` (lightweight decision logs), not as full ADRs.

## Naming

```
NNNN-kebab-title.md
```

`NNNN` is a zero-padded four-digit sequence number. The first real ADR is
`0001-...`; `0000-template.md` is the template and is never a real decision.

Example: `0001-storage-format-object-layout.md`

## Lifecycle

```
proposed  →  reviewed (adversarial)  →  accepted  →  [superseded-by NNNN]
```

1. **Proposed** — author writes the ADR, sets `status: proposed`, opens a
   board item for review (`status: in_review`).

2. **Reviewed (adversarial)** — one or more `adversarial-reviewer` agents run
   the design falsification loop (see
   [docs/process/adversarial-review-loops.md](../process/adversarial-review-loops.md)).
   Blocking findings must be resolved before ratification. The ADR is updated
   with each round's outcome.

3. **Accepted** — the relevant steering-committee member(s) sign off by
   appending their ratification entry to the ADR's Sign-off section
   (see [docs/process/steering-committee.md](../process/steering-committee.md)).
   `status` is updated to `accepted`. The corresponding `deps` on implementation
   tasks now clear, and those tasks become `ready`.

4. **Superseded** — if a later ADR replaces this one, set
   `status: superseded-by NNNN` and add a forward link. The old ADR is never
   deleted — it is the historical record of why the earlier decision was made.

## Ratification

ADRs are ratified by the **steering committee** after surviving adversarial
review. See [docs/process/steering-committee.md](../process/steering-committee.md)
for membership and quorum rules.

A steering sign-off is a committed, explicit record appended to the ADR's
Sign-off section. It is not implied by silence and not replaceable by a chat
message.

## Template

Use [0000-template.md](0000-template.md) for every new ADR. Copy it, rename
it to the next sequence number and a descriptive kebab title, and fill in all
sections before requesting adversarial review.
