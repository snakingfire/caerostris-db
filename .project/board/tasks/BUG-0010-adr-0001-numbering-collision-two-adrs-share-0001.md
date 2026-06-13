---
id: BUG-0010
title: ADR numbering collision — two ADRs share 0001 (latency-envelope and cold-start-benchmark-protocol)
type: bug
status: ready
priority: P2
assignee:
epic: EPIC-010
deps: []
rubric_refs: [12]
estimate: S
created: 2026-06-13T19:52:00Z
updated: 2026-06-13T19:52:00Z
---

## Context

Found by `steering-formal-methods` during the SPIKE-0001 ratification pass
(decision `0015`). `docs/adr/` now contains **two** ADRs numbered `0001`:

- `docs/adr/0001-latency-selectivity-envelope.md` (SPIKE-0001)
- `docs/adr/0001-cold-start-benchmark-protocol.md` (SPIKE-0007)

The ADR README mandates a unique zero-padded sequence number per ADR. Two ADRs at
the same number breaks the index, ambiguates cross-references, and risks the
rubric-grader mis-attributing Cat. 3 / Cat. 10 evidence. This is docs/board hygiene
(Cat. 12), **not** a design issue — it does not affect the ratified latency theorem.

Lower-churn fix: renumber the **cold-start-benchmark-protocol** ADR (fewer inbound
references — 2, both in `docs/process/testing-and-benchmarks.md`) to the next free
ADR number, leaving the more widely-referenced latency-selectivity-envelope ADR at
0001. (Renumbering the envelope ADR instead would break ~6+ references across
decisions, reports, and board items.)

## Acceptance criteria
- [ ] One of the two `0001` ADRs renumbered to a free ADR number (recommend renumber
      `0001-cold-start-benchmark-protocol.md`).
- [ ] All inbound references updated (grep `0001-cold-start-benchmark-protocol`:
      currently `docs/process/testing-and-benchmarks.md` ×2; re-grep after move).
- [ ] ADR README index (if it enumerates ADRs) reflects the new number.
- [ ] `./format_code.sh` green (markdown-only change; should be a no-op for fmt/clippy).

## Notes / log
- 2026-06-13T19:52Z `steering-formal-methods`: filed as finding F3 of the SPIKE-0001
  ratification (decision `0015`). Non-blocking for the latency-envelope ratification.
  Owner: docs-memory-curator. Do NOT renumber `0001-latency-selectivity-envelope.md`
  (it is the canonical, widely-referenced artifact).
