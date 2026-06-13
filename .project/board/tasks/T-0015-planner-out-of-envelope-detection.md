---
id: T-0015
title: Implement plan-time out-of-envelope detection (reject/warn/degrade)
type: task
status: backlog
priority: P1
assignee:
epic: EPIC-003
deps: [SPIKE-0001, SPIKE-0004, T-0009]
rubric_refs: [3, 4]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

The non-negotiable invariant: out-of-envelope queries are detected at plan time and
**never silently miss the SLA**. The planner estimates projected bytes-read and
tail fan-out in O(plan-size) from the manifest statistics (SPIKE-0004 / decision
0009 — using a tail/worst-case fan-out, not the mean, so a super-node is correctly
flagged) before any object-store access, then rejects / warns / degrades per
SPIKE-0001's specified response. Design-gated on SPIKE-0001 + SPIKE-0004, and needs
the manifest statistics block (T-0009). See `EPIC-003`, `EPIC-002`.

## Acceptance criteria
- [ ] Estimator computes projected bytes-read and tail fan-out for a 6-hop plan from manifest statistics in O(plan-size), before any object-store access.
- [ ] Uses the tail/worst-case fan-out term (SPIKE-0004); a super-node example is correctly classified out-of-envelope (tested).
- [ ] When stats are missing/stale, the planner defaults to conservative reject/warn — never optimistic accept (SPIKE-0004 rule) — tested.
- [ ] In-envelope queries pass; out-of-envelope queries produce the SPIKE-0001-specified explicit response (clear error / warning / degraded plan), never a silent SLA miss.
- [ ] tests added (unit on representative in/out-of-envelope plans); coverage not regressed
- [ ] docs / ADR updated if behaviour changed
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: blocked on SPIKE-0001 + SPIKE-0004 ratification and T-0009
(manifest statistics). This is the planner half of the Cat. 3 GATE.

- **T+~01:28 steering-formal-methods ratification conditions (decision 0015, ADR 0001):**
  When SPIKE-0001 ratification completes, these two conditions are BINDING on this task:
  - **F1 (α-corrected OOE-4 thresholds):** the deployment-latency check must use
    `(T − T_compute)/(K_min·α)`, i.e. **102 ms** for the 1 s target and **216 ms** for
    the 2 s ceiling — NOT the α-free 112.5 / 237.5 ms figures printed in ADR §1.4 / §4.2
    (those drop the α=1.10 max-of-M factor and are optimistic). Add a test asserting a
    deployment at L_p99=230 ms is flagged (it busts the α-aware 2 s ceiling).
  - **F2 (max-degree byte safety bound):** the byte estimator must size the adjacency-byte
    safety bound from a hard per-GET byte cap (early-abort) or the per-rel-type **max**
    out-degree (from SPIKE-0004's contract), NOT the p99. A frontier whose degree band
    includes a node above the maintained max-degree statistic must be conservatively
    rejected (same doctrine as OOE-5). Add a super-hub test where p99 admits but max
    rejects.
