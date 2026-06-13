---
id: SPIKE-0008
title: Storage-domain falsification constraints from ratification pass
type: spike
status: in_progress
priority: P0
assignee: researcher
epic: EPIC-001
deps: []
rubric_refs: [2, 3, 1]
estimate: S
created: 2026-06-13T18:29:56Z
updated: 2026-06-13T19:05:00Z
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

- [ ] **F1 — Early-abort partial adjacency reads are mandatory, not optional
      (binding 50 Mbps case).** The intent's own degree-10 / 6-hop example gives
      fan-out^6 = 1e6. With B_max ~= 4 MB at 50 Mbps and even ~16 B/node, the naive
      product bound `|seed| <= B_max / (node_bytes * fan_out^6)` evaluates to **< 1
      seed node** — i.e. a full breadth-first 6-hop expansion of even a single seed
      does not fit. The 50 Mbps envelope is therefore feasible **only** if
      `LIMIT`-driven early termination prunes the realized frontier far below
      fan_out^6 AND the on-object layout lets a reader **abort an adjacency-list
      range-GET early** (stop reading once LIMIT is satisfied) rather than fetching
      whole adjacency objects. SPIKE-0003 must specify adjacency-list chunking /
      page sizing that bounds the *minimum* committed read per hop so early
      termination actually saves bytes. SPIKE-0001 must state the realized-fan-out
      assumption (not just worst-case product) its proof relies on, and SPIKE-0003
      must show the layout supports it. (rubric_refs: 3, 2)

- [ ] **F2 — Atomic manifest swap depends on a conditional-PUT primitive that must
      be pinned, not assumed.** Cat. 1 & Cat. 2 make atomic commit a GATE. The
      "single conditional PUT (if-none-match) = compare-and-swap" mechanism is real
      on modern S3 (conditional writes, 2024+) and on MinIO, but is **not universal
      across all S3-compatible stores or all mock configurations**. SPIKE-0002 /
      SPIKE-0003 must name the exact primitive used (If-None-Match / If-Match /
      versioned-PUT + read-back), confirm the local mock (MinIO/moto) supports it,
      and specify the fallback (or hard precondition) if a target store does not.
      If the chosen mock does not honor conditional-PUT semantics, the GATE
      atomicity claim is unprovable on it — flag immediately to the joint
      storage+distributed-acid session. (rubric_refs: 1, 2)

- [ ] **F3 — GC must be safe against slow/crashed readers with no central pin
      registry.** Cat. 2's "100" anchor requires "manifest swap atomic &
      concurrent-reader-safe," and R3 mode 3 (master-less) + embedded read-only
      readers mean there is no always-live coordinator the GC can cheaply consult.
      The intent/rubric say "readers pin a version; old versions readable until GC"
      but do not address: (a) a reader that crashed mid-read leaving a stale pin,
      (b) a slow reader whose pin GC cannot see, or (c) GC deleting an object a
      reader is mid-range-GET on. SPIKE-0002/3 must specify a **safe-GC policy** —
      e.g. a minimum version-retention grace window, generational manifest
      retention, or lease-TTL'd pin objects with a deletion deadline strictly after
      the max reader-session lifetime — that provably prevents GC from deleting an
      object any non-expired reader could still reference. The TLA+ model
      (SPIKE-0002) should include a GC-vs-reader interleaving invariant.
      (rubric_refs: 1, 2)

- [ ] Cross-reference: SPIKE-0003 and SPIKE-0002 each cite the finding(s) they
      discharge; this item is closed by `steering-storage` once all three are
      addressed in the ratified artifacts.

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
