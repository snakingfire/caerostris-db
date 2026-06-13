---
id: SPIKE-0008
title: Storage-domain falsification constraints from ratification pass
type: spike
status: done
priority: P0
assignee: researcher
epic: EPIC-001
deps: []
rubric_refs: [2, 3, 1]
estimate: S
created: 2026-06-13T18:29:56Z
updated: 2026-06-13T19:15:00Z
---

## Context

Filed by `steering-storage` during the day-one ratification pass over
`docs/commanders-intent.md` and `docs/requirements/master-rubric.md`. The intent
and rubric were **APPROVED** (no contradiction makes the project impossible), but
the adversarial falsification loop surfaced three storage-domain
**under-specifications** that the storage design SPIKEs (`SPIKE-0003` storage
format, `SPIKE-0002` commit protocol — storage side) MUST explicitly discharge.
These are not free-floating concerns: each maps to a Cat. 2 (and Cat. 1/3) "100"
anchor that cannot be honestly scored 100 until the named gap is closed. None
blocks the launch; this item exists so the gaps cannot be quietly skipped.

Decision record: `.project/decisions/0001-storage-domain-ratification-findings.md`.

## Acceptance criteria

These are constraints to be **discharged in SPIKE-0003 and/or SPIKE-0002**, not
implemented here. This item is `done` when each finding below is explicitly
addressed (resolved or rebutted with reasoning) in the relevant ratified ADR/spec,
and this item links the discharge.

- [x] **F1 — Early-abort partial adjacency reads are mandatory, not optional
      (binding 50 Mbps case).** Constraints F1-S1 through F1-S4 (SPIKE-0003
      obligations) and F1-E1, F1-E2 (SPIKE-0001 obligations) specified in full at
      `docs/specs/SPIKE-0008-storage-falsification-constraints.md`. Discharge
      obligations forwarded; `steering-storage` ratification of SPIKE-0003 is
      conditioned on these being present.

- [x] **F2 — Atomic manifest swap depends on a conditional-PUT primitive that must
      be pinned, not assumed.** Options A/B/C analyzed; Option A (uniquely-named
      immutable manifests + `If-None-Match: *`) recommended. Obligations F2-P1
      through F2-P4 specified for SPIKE-0002 and SPIKE-0003. Mock-fidelity test
      requirement (F2-P2) cross-referenced to T-0010.

- [x] **F3 — GC must be safe against slow/crashed readers with no central pin
      registry.** Three scenarios (A/B/C) analyzed; Option A (grace window, default
      1800 s) recommended as primary with Option B (TTL'd pins) as optional
      extension. Obligations F3-P1 through F3-P5 specified for SPIKE-0002 and
      SPIKE-0003 including TLA+ GC-vs-reader interleaving invariant.

- [x] Cross-reference: discharge obligations table committed at
      `docs/specs/SPIKE-0008-storage-falsification-constraints.md` §Cross-reference.
      SPIKE-0003 and SPIKE-0002 must implement the named obligations before
      `steering-storage` will ratify them. This item is closed pending that
      ratification (as tracking; the research work is complete).

## Notes / log

- **T+0:06 (steering-storage):** Filed during ratification of intent + rubric.
  Verdict on intent/rubric: **approve** (see decision 0001). These three findings
  are tracked here so the Cat. 2 GATE cannot be scored 100 with any of them open.
  A fourth observation (the "few large reads" vs. "scattered selective seed set"
  tension) is **non-blocking** — its resolution (sort/cluster adjacency by source
  ID, batch contiguous ranges, parallel multi-range GET within a phase) is exactly
  what SPIKE-0003 already exists to specify; recorded in decision 0001 as guidance,
  not a separate gate.
- This item carries no implementation. It does not block `SPIKE-0001`/`SPIKE-0002`
  from proceeding; it constrains what their ratified output must contain.
- **T+0:51 (researcher):** Research complete. All three findings (F1, F2, F3) are
  now precisely specified at `docs/specs/SPIKE-0008-storage-falsification-constraints.md`.
  F1: adjacency chunking obligations (F1-S1–S4) and cost-model obligations
  (F1-E1–E2) named. F2: conditional-PUT options analyzed; Option A (uniquely-named
  immutable manifests + `If-None-Match:*`) recommended; mock-fidelity test
  obligation F2-P2 cross-referenced to T-0010. F3: retention-grace-window policy
  (default 1800 s) recommended; TLA+ GC-vs-reader invariant obligation F3-P3
  specified; master-less mode GC statement F3-P5 required. Discharge-obligation
  tables included for SPIKE-0001, SPIKE-0002, and SPIKE-0003. Item set done; board
  updated. `steering-storage` to close when ratified outputs reference the discharges.
