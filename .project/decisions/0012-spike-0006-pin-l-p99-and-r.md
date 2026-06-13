# Decision 0012 — Pin L_p99 = 50 ms and r = 1 as first-class latency-envelope parameters

- **Date / marker:** 2026-06-13 (T0+~01:00)
- **Author / role:** researcher (SPIKE-0006)
- **Type:** research finding + recommendation (design-level; feeds SPIKE-0001 and SPIKE-0003)
- **Status:** recommended; steering sign-off pending (steering-perf-sla, steering-formal-methods)
- **Rubric:** Cat. 3 (latency envelope, GATE, w14), Cat. 11 (formal artifacts, GATE, w6)
- **Affects:** SPIKE-0001 (envelope spec), SPIKE-0003 (storage format constraint), EPIC-003
- **Full spec:** `docs/specs/SPIKE-0006-l-p99-and-per-hop-round-trip-bound.md`

## Summary

SPIKE-0006 was filed by steering-perf-sla (decision 0010) because the latency envelope
cost model uses `K * L_p99` as a serial latency floor without naming the assumed `L_p99`
or bounding the per-hop round-trip count `r`. This decision pins both.

## Findings

1. **Evidence-based L_p99 range for S3 Standard (same-region EC2):**
   - P50: 20–45 ms (independent benchmarks; 26 ms in TopicPartition, 30 ms in Quickwit, 45 ms in us-east-1 aggregates)
   - P99: 86–200 ms (86 ms at TopicPartition eu-north-1; "100+ ms" at Nixiesearch; 200 ms in us-east-1 aggregates)
   - Tail events above 200 ms exist but are sparse (S3 retries/redirects)
   - S3 Express One Zone: P50 2–5 ms, P99 5–10 ms (premium, AZ-pinned variant)

2. **K_min is structurally determined by the query shape:**
   - K_min = 1 (manifest pin) + 1 (index probe) + 6 * r
   - At r = 1: K_min = 8
   - At r = 2: K_min = 14

3. **Latency floor `K_min * L_p99` dominates the budget at the top of the measured range:**
   - K=8, L_p99=150 ms: floor = 1200 ms (busts the 1 s target before a byte moves)
   - K=14, L_p99=150 ms: floor = 2100 ms (busts the 2 s ceiling)
   - K=8, L_p99=50 ms: floor = 400 ms (leaves 600 ms for transfer + compute — headline case)

## Decision

**Pin the following as first-class, named parameters in SPIKE-0001's envelope spec:**

| Parameter | Value | Basis |
|-----------|-------|-------|
| L_p99 (headline) | **50 ms** | Evidence-based P90–P95 of same-region S3 Standard GET (reproduces ~75 MB / ~4 MB headlines) |
| r (round-trips per hop) | **1** | Storage-format hard constraint fed to SPIKE-0003 |
| K_min | **8** | 1 + 1 + 6×1 |
| T_floor = K_min * L_p99 | **400 ms** | Presented as a separate line item in the cost model |
| 2 s ceiling worst-case L_p99 | **≤ 250 ms** | 8 × 250 = 2000 ms; survives all measured S3 Standard P99 values |
| Deployment-check threshold (warn) | L_p99 > 125 ms | 8 × 125 = 1000 ms floor — 1 s target at risk |
| Deployment-check threshold (reject) | L_p99 > 250 ms | 8 × 250 = 2000 ms floor — 2 s ceiling at risk |

## What SPIKE-0001 must do

1. Present `T_total = T_floor + T_transfer + T_compute` with `T_floor = K_min * L_p99` as
   a named, explicit line item — not folded into a single residual.
2. Annotate the ~75 MB / ~4 MB byte budgets with the (K=8, L_p99=50 ms, T_compute) triple.
3. Add deployment-check detection: L_p99 > 125 ms = warn; > 250 ms = hard reject.
4. Cross-reference `r = 1` as a storage-format constraint forwarded to SPIKE-0003.
5. Combine the floor term established here with the max-of-M amplification term from
   decision 0005 / BUG-0004 in the same cost model.

## What SPIKE-0003 must do

Receive `r = 1` as a hard requirement: adjacency offsets must be co-located with or
derivable from the adjacency payload in a single range GET. If r = 1 cannot be achieved
by the storage layout, escalate to steering before SPIKE-0001 ratification.

## Alternatives considered

- **L_p99 = 100 ms:** shrinks byte budget to ~12.5 MB at 1 Gbps; diverges from the committed
  ~75 MB headline; narrows the envelope to the point of practical uselessness at 50 Mbps. Rejected.
- **L_p99 = 20 ms (S3 Express profile):** anchors the theorem to a premium, AZ-pinned storage
  class; inconsistent with "commodity S3" framing; violates the "fast only with expensive
  hardware" corollary of the "fast only when warm" invariant. Rejected.

## Risks

- L_p99 = 50 ms is below the measured P99 (86–200 ms). The theorem holds at the operating
  point, not at extreme tail events. This must be documented clearly in SPIKE-0001 and the
  benchmark injected-latency profile must use 50 ms P99 (per SPIKE-0007).
- If SPIKE-0003 finds r = 1 is not achievable, K jumps to 14 and the 1 s target is infeasible
  at any measured S3 P99. Escalate immediately.
- The compute reserve (50–100 ms) is an estimate; it must be validated by Cat. 10 benchmarks.

## Steering sign-off required

- steering-perf-sla: ratify L_p99 = 50 ms, deployment-check boundaries, and byte-budget annotation.
- steering-formal-methods: ratify T_floor line-item placement and ordering with max-of-M term.
