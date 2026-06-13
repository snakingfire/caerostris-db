# Decision 0012 — SPIKE-0005 Steering Sign-Off Request

- **Date:** 2026-06-13T19:10:00Z
- **Owner:** researcher (SPIKE-0005)
- **Type:** steering ratification request (design artifact)
- **Status:** PENDING — awaiting steering sign-off
- **Routing:** `steering-distributed-acid` (primary), `steering-formal-methods` (secondary)
- **Related:** SPIKE-0005, SPIKE-0002, EPIC-001, EPIC-004, Cat. 1, Cat. 7, Cat. 11

## What is being ratified

The research output for SPIKE-0005 is committed at:
`docs/specs/SPIKE-0005-commit-protocol-pre-ratification-constraints.md`

This document provides concrete resolutions for the three pre-ratification
constraints that `steering-distributed-acid` required SPIKE-0002 to address:

1. **Constraint 1 — CAS primitive + mock fidelity:** Recommends `If-None-Match: *`
   (create-if-absent) with uniquely named immutable manifest objects and
   lexicographic-max list resolution. Specifies a concrete mock-fidelity
   integration test. Rejects `If-Match` on PUT due to moto mock fidelity risk.

2. **Constraint 2 — Fencing token in swap predicate:** Recommends embedding the
   generation counter in the manifest key name, so the swap predicate IS the
   uniqueness of the key via `If-None-Match: *`. Restates the safety invariant as
   "at most one manifest object per generation N" (not `writer_count <= 1`).
   Specifies a ZombieWriter process for the TLA+ model.

3. **Constraint 3 — Durability ordering barrier:** Recommends strict write ordering
   (all data object PUTs acked before manifest swap issued; client ack = swap ack).
   Specifies `DataObjectDurable` predicate, reader-safety invariant, and recovery
   invariant for the TLA+ model.

## Sign-off gate

This research is a design artifact and must be ratified by steering before the
SPIKE-0002 ADR revisions (which incorporate these resolutions) are themselves
ratified and before any commit-path implementation task becomes `ready`.

The ratification bar:
- `steering-distributed-acid`: confirm the three constraint resolutions are
  structurally sound and satisfy the safety requirements you set in SPIKE-0005
  and `.project/decisions/0004-distributed-acid-ratification-findings.md`.
- `steering-formal-methods`: confirm the TLA+ model obligations (ZombieWriter
  process, `ManifestVersionUniqueness` invariant, `DataObjectDurable` predicate,
  reader-safety and recovery invariants) are correctly specified and modelable
  within the Apalache bounded model-checking constraints.

## Ratification record

<!-- Append sign-off entries here once steering members review. -->

### steering-distributed-acid

_(pending)_

### steering-formal-methods

_(pending)_

## What happens after ratification

1. SPIKE-0005 status is updated from `in_review` to `done`.
2. The SPIKE-0002 author revises the commit-protocol ADR to incorporate the
   three constraint resolutions documented in SPIKE-0005.
3. The revised SPIKE-0002 ADR is submitted for adversarial review and then
   steering ratification (`steering-distributed-acid` + `steering-formal-methods`).
4. Once SPIKE-0002 is ratified, commit-path implementation tasks in EPIC-001
   and EPIC-004 become `ready`.
