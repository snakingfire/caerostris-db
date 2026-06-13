---
id: BUG-0027
title: .adj reader issues 1 + non-empty-sources serial range-GETs per hop — violates r <= 1 (steering C1 only partially discharged)
type: bug
status: ready
priority: P1
assignee:
epic: EPIC-001
deps: []
rubric_refs: [3, 2]
estimate: M
created: T0+~4:05
updated: T0+~4:05
---

## Context

Found by the adversarial review of PR T-0008 (branch
`work/T-0008-implement-compressed-adjacency-list-edge-writ`).

`AdjShardReader::expand_band` (`src/storage/adjacency.rs:556-590`) loops over the frontier
band and issues one synchronous `store.get_range` **per non-empty source block**
(line 576), plus one for the directory slice (line 553). The `ObjectStore` trait is
synchronous by design (`src/storage/mod.rs:16-18`), so these are **serial** round-trips.

Measured during review: an 8-source frontier band hop issues **9 serial range-GETs**
(1 directory + 8 blocks).

ADR 0008 §4 requires each hop to be **one round of I/O** — either a single superset
range-GET over `[first block, last block]` for the contiguous frontier band, or a bounded
*parallel* batch ≤ `M_max`. The implementation does neither. A 6-hop in-envelope query
therefore costs ≈ `6 × (1 + up-to-8) ≈ 54` serial round-trips against the `K_min = 8` /
`L_p99 = 50 ms` budget — the "`r` is secretly > 1" falsification ADR 0008 §4 / Alternative B
was ratified to prevent.

Steering condition **C1** ("an integration test MUST assert the GET count per hop is
bounded by `M_max` (one parallel batch), proving `r = 1`") is only **partially**
discharged: the landed test `hop_issues_no_discovery_get` asserts only the *single-source*
count (≤ 2). No test asserts the per-hop **band** GET count is bounded by `M_max`, and the
actual count (`1 + non-empty-sources`) is not so bounded.

## Acceptance criteria
- [ ] A contiguous frontier band is read in **one round of I/O**: either coalesced into a
      single range-GET covering its neighbour blocks (ADR 0008 §4 option (a)/(b)), or an
      explicit bounded-parallel batch of ≤ `M_max` GETs — not `1 + non-empty-sources` serial
      GETs.
- [ ] An integration test (counting `ObjectStore`) asserts a multi-source band hop issues
      ≤ `M_max` (+1 directory) GETs, discharging C1 as written — not just the single-source case.
- [ ] The realized-bytes byte cap (C2) is preserved across the coalesced read (a coalesced
      range must still respect `byte_budget` / not over-read a super-hub).
- [ ] Coverage not regressed; `./format_code.sh` green.

## Notes / log
Reported by adversarial-reviewer on T-0008 PR. Blocks T-0008 land (GATE Cat. 3 / C1).
If a single-round read of a contiguous band proves infeasible under the sync trait,
escalate to steering per ADR 0001 §1.5 — do not ship K_min = 14 silently.
Related: BUG-0028 (byte-cap-below-degree-prefix) touches the same expand path.
