---
id: T-0010
title: Implement atomic manifest-swap commit with CAS predicate + durability barrier
type: task
status: backlog
priority: P1
assignee:
epic: EPIC-004
deps: [SPIKE-0002, SPIKE-0005, T-0009]
rubric_refs: [1, 2, 11]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

The write path: stage all data objects for version V+1, fully PUT and durably
confirm them, then issue the manifest swap as a CAS conditional on the **current
manifest version/etag** (per `SPIKE-0005` Constraint 2 — safety is CAS-on-manifest,
not lease belief). The commit ack to the client is the manifest-swap ack
(`SPIKE-0005` Constraint 3, durability barrier). The Rust implementation must match
the TLA+ model from SPIKE-0002 — drift is a bug. Design-gated on SPIKE-0002 +
SPIKE-0005 ratification. See `EPIC-004`, `formal/commit-protocol/`.

## Acceptance criteria
- [ ] Commit stages data objects, confirms durable PUT of every object referenced by V+1 before issuing the swap, then swaps the manifest via the CAS primitive named in SPIKE-0002's ADR.
- [ ] Safety invariant holds: at most one commit succeeds per predecessor manifest version (concurrent committers on the same predecessor → exactly one wins) — tested with two racing committers on the mock.
- [ ] Zombie/fenced-writer scenario: a stale writer (lease expired) attempting a swap on a superseded manifest is rejected by the CAS predicate — tested.
- [ ] Commit-phase sequence in the code matches the SPIKE-0002 TLA+ model's phase ordering; a comment or doc cross-references the model.
- [ ] tests added (unit + integration on the mock; property tests for interleavings); coverage not regressed
- [ ] docs / ADR updated; TLA+ model kept in sync (note any model change)
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: blocked on SPIKE-0002 + SPIKE-0005. This is the Cat. 1 GATE
core. Keep the implementation phase-for-phase with the model.
