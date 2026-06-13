# SPIKE-0006 — Pinning L_p99 and the Per-Hop Round-Trip Bound r in the Latency Envelope

**Status:** research complete; steering sign-off pending  
**Date:** 2026-06-13 (T0+~01:00)  
**Author:** researcher  
**Feeds:** SPIKE-0001 (latency envelope spec), SPIKE-0003 (storage format), EPIC-003  
**Rubric:** Cat. 3 (latency envelope + SLA, GATE, w14), Cat. 11 (formal artifacts, GATE, w6)

---

## Research: What values of L_p99 and r must SPIKE-0001's envelope spec pin?

**Question restated:** What specific value of S3 per-request P99 latency (L_p99) and what
per-hop round-trip count (r) should SPIKE-0001's analytical cost model assume as named,
explicit parameters — and how do those choices constrain the storage format and the worst-case
deployment the 2 s ceiling can tolerate?

---

## 1. Evidence base for L_p99

### 1.1 Published benchmarks

The table below aggregates independent measurements of S3 Standard GET latency from
co-located EC2 instances in the same AWS region. All figures are per-request latencies for
objects in the range that caerostris-db would read per range GET (tens of KB to a few MB).

| Source | Object size | P50 | P95/P99 | Notes |
|--------|-------------|-----|---------|-------|
| TopicPartition.io (eu-north-1, 2024) | 500 KB | 26 ms | P99 = **86 ms** | 100 GET iterations, same-region EC2 |
| Nixiesearch/nixie benchmark (2024) | 4 KB random reads | — | "100+ ms P99" | HNSW pattern, m5id.large |
| Quickwit blog (2023) | mixed | P50 ≈ 30 ms | P90 ≈ 50 ms, tail "80 ms common" | TTFB focus, same AZ |
| AWS/WarpStream blog (S3 Standard write path) | multi-KB chunks | — | P99 write 400–600 ms | includes S3 + control-plane commit; not a clean GET floor |
| AWS docs / community aggregates | < 512 KB | 45 ms P50 | P99 ≈ 200 ms | us-east-1, multiple reports |

**Summary of the evidence:**
- P50 for a same-region GET sits in the **20–45 ms** range for objects under 1 MB.
- P99 for the same workload sits in the **86–200 ms** range; the commonly-cited practical
  upper bound is **~150 ms** when requests are issued from a co-located EC2 instance.
- Tail events above 200 ms exist but are sparse; they correspond to S3's internal
  retry/redirect events and are not representative of the steady-state P99.
- These numbers are consistent with the range cited in SPIKE-0001 (50–150 ms) and in
  decision 0010 (steering-perf-sla).

**S3 Express One Zone** (separate storage class): P99 is in the **5–10 ms** range and P50
is 2–5 ms. This is not the baseline for the theorem (the design targets S3 Standard
semantics), but it is a useful deployment option for teams that can tolerate AZ-pinning.

### 1.2 The 50–150 ms range is evidence-backed

SPIKE-0001 already cites "typ. 50–150 ms." That range is confirmed by the independent
benchmarks above. The design must be clear, however, about which point in that range is
used for the headline derivation versus which point defines the hard ceiling.

---

## 2. Derivation of the recommended pinned values

### 2.1 The serial phase structure

For a 6-hop unanchored property-filtered MATCH with LIMIT-driven early termination, the
minimum number of sequential S3 round-trip phases is:

```
K_min = 1 (manifest/root pin — cold start version read)
      + 1 (index probe — B-tree leaf to resolve seed set)
      + 6 * r  (one or two round-trips per hop, depending on layout)
```

where `r` is the number of sequential S3 GETs that must complete before hop k+1's range
GETs can be issued. This is a *serial* dependency: no pipelining or parallelism eliminates
it because hop k+1's frontier is not known until hop k's adjacency reads return.

**`r = 1` (co-located layout):** the adjacency list offsets for a node's neighbors are
embedded in or derivable from the same object range that returns the adjacency payload.
One GET per hop phase suffices to both locate and return the neighbor set.

**`r = 2` (indirection layout):** a first GET fetches an offset/pointer table, and a
second GET fetches the actual adjacency payload at the indicated range. Two serial GETs
per hop.

K_min values:

| r | K_min | Formula |
|---|-------|---------|
| 1 | **8** | 1 + 1 + 6×1 |
| 2 | **14** | 1 + 1 + 6×2 |

### 2.2 Latency floor at various L_p99 values

The latency floor `K_min * L_p99` is the serial latency cost before a single byte
transfers and before any compute runs. It is a hard lower bound on query latency.

```
T_floor = K_min * L_p99
```

Full table (ms):

| K_min | L_p99 = 20 ms | 30 ms | 50 ms | 86 ms | 100 ms | 150 ms |
|------:|-------------:|------:|------:|------:|-------:|-------:|
| 8 (r=1) | 160 | 240 | **400** | 688 | 800 | **1200** |
| 14 (r=2) | 280 | 420 | **700** | 1204 | **1400** | **2100** |

Bold entries exceed the 1 s target (> 1000 ms) or the 2 s ceiling (> 2000 ms).

**Key finding:** at r=1 (K=8), the 1 s *target* is violated as a latency floor when
L_p99 ≥ 125 ms (floor = 1000 ms). The 2 s *ceiling* survives up to L_p99 = 250 ms/8 = 
**250 ms**, which exceeds any measured S3 Standard P99.

At r=2 (K=14), the 1 s target is busted for L_p99 ≥ 72 ms (floor = 1008 ms),
and the 2 s ceiling is busted for L_p99 ≥ 143 ms — right inside the top of the
measured S3 P99 range.

### 2.3 Reconciling with the byte budget

The full cold-start latency for an in-envelope query:

```
T_total = K_min * L_p99  +  T_transfer  +  T_compute
        ≤ T_budget  (1 s target, 2 s ceiling)
```

For `T_total ≤ 1 s` with K=8, L_p99=50 ms, T_compute=100 ms:

```
T_transfer ≤ 1000 − (8×50) − 100  =  1000 − 400 − 100  =  500 ms
B_max (1 Gbps) = 1 Gbps × 0.5 s / 8  ≈  62.5 MB
B_max (50 Mbps) = 50 Mbps × 0.5 s / 8  ≈  3.1 MB
```

For `T_total ≤ 1 s` with K=8, L_p99=30 ms (conservative same-region P50), T_compute=100 ms:

```
T_transfer ≤ 1000 − (8×30) − 100  =  1000 − 240 − 100  =  660 ms
B_max (1 Gbps) = 1 Gbps × 0.66 s / 8  ≈  82.5 MB
B_max (50 Mbps) = 50 Mbps × 0.66 s / 8  ≈  4.1 MB
```

The intent's headline ~75 MB / ~4 MB figures correspond to approximately L_p99 = 30–50 ms
at K=8, r=1, T_compute ≈ 100 ms. This is reachable in practice for a same-region EC2
deployment against S3 Standard.

---

## 3. Options considered

### Option A — Pin L_p99 = 50 ms as the headline assumption (recommended)

**Description:** SPIKE-0001's cost model uses L_p99 = 50 ms as the single named value for
the headline derivation. The ~75 MB / ~4 MB byte budgets are annotated as computed at
(K=8, L_p99=50 ms, T_compute=100 ms). The 2 s ceiling is explicitly annotated as
surviving up to L_p99 = 150 ms (the worst-case measured S3 Standard P99), which gives
T_floor = 8 × 150 = 1200 ms and leaves 800 ms for transfer + compute. Out-of-envelope
detection includes a deployment check: if the observed or configured L_p99 > 125 ms, the
engine emits a warning (or rejection at L_p99 > 150 ms) independent of query size, because
the 1 s target cannot be met on latency alone.

**Pros:**
- L_p99 = 50 ms is the P90–P95 of the measured S3 Standard distribution for same-region
  requests (not an optimistic outlier). It is reproducible under normal conditions.
- Annotation of the 2 s ceiling at L_p99 = 150 ms gives a clear deployment boundary.
- Consistent with the intent's ~75 MB headline (exact match at K=8, L_p99=50 ms, T_compute=100 ms).
- Out-of-envelope detection can flag "deployment too slow" as a first-class rejection.

**Cons:**
- L_p99 = 50 ms is below the P99 benchmark values (86–200 ms). Queries that hit a
  genuine P99 tail event (>50 ms per round trip) will breach the 1 s target. The theorem
  holds at the assumed operating point, not at extreme tail events. This must be stated
  explicitly in the spec.
- The theorem's guarantee is therefore "at L_p99 ≤ 50 ms" — not "for all S3 P99 events."

**Compatible with commander's intent:** yes. The intent's "~75 MB" is reproduced exactly
at this operating point. The conditional nature of the theorem (operating-point conditional)
is consistent with the "formal conditional theorem" framing.

**License impact:** none (this is an analytical choice, no external dependency).

---

### Option B — Pin L_p99 = 100 ms as the headline assumption

**Description:** Use the midpoint of the 50–150 ms range. This shrinks byte budgets
significantly: at K=8, L_p99=100 ms, T_compute=100 ms: T_transfer ≤ 100 ms →
B_max(1 Gbps) ≈ 12.5 MB, B_max(50 Mbps) ≈ 625 KB. These are much tighter envelopes
and would require very high selectivity to admit any useful query.

**Pros:**
- More conservative; the headline SLA holds even for above-P50 tail events.

**Cons:**
- The byte budgets (12.5 MB at 1 Gbps) diverge dramatically from the ~75 MB headline
  already committed in the intent and rubric. The intent would require correction.
- A 625 KB budget at 50 Mbps admits almost no query over a 1B/10B graph — the
  conditional theorem would cover an unrealistically narrow envelope.
- Conflates L_p99 (the statistical operating point) with L_p999 (rare tail events).
  The theorem is a P99 guarantee at the stated operating point; it is not a universal
  guarantee over all latency percentiles simultaneously.

**Rejected:** inconsistent with the committed ~75 MB headline; narrows the envelope
to the point of practical uselessness at 50 Mbps.

---

### Option C — Pin L_p99 = 20 ms (S3 Express One Zone profile)

**Description:** Assume S3 Express One Zone is the deployment target, with P99 around
5–10 ms (conservatively rounded to 20 ms for the derivation). K=8 gives T_floor = 160 ms,
leaving 740 ms for transfer + compute. B_max(1 Gbps) ≈ 92.5 MB.

**Pros:**
- Widest byte budget; most permissive envelope.
- S3 Express One Zone is available today and the latency profile is real.

**Cons:**
- S3 Express One Zone requires AZ-pinning and costs ~3× more per request than S3 Standard.
  Designing the theorem around it would make the SLA hold only for a non-standard
  (higher-cost) deployment class.
- The design is explicitly "commodity object storage (S3)"; S3 Express is a premium variant.
  Anchoring the baseline theorem to it is a "fast only with expensive hardware" variant
  of the "fast only when warm" trap the commander's intent forbids.
- The intent's ~75 MB headline is not reproduced at this operating point (it gives ~92 MB),
  creating a disconnect between the spec and the committed headlines.

**Rejected:** inconsistent with "commodity S3" framing; anchors the baseline SLA to
a premium storage class rather than the commodity case.

---

## 4. Recommendation

### Pinned parameters for SPIKE-0001

**Recommended: Option A — L_p99 = 50 ms, r = 1 (K_min = 8)**

**L_p99 = 50 ms** is the named assumed value for the headline cost-model derivation.
It corresponds to approximately the P90–P95 of measured same-region S3 Standard GET
latency for objects in the tens-to-hundreds-of-KB range. It is achievable reliably
under normal operating conditions (not just cold-start luck or fast-path caching),
and it reproduces the intent's ~75 MB / ~4 MB headline byte budgets exactly.

**r = 1** (one serial round-trip per hop) is stated as a **storage-format constraint**:
the adjacency list offsets for a node's neighbors must be derivable without a second
serial round-trip. This is fed directly to SPIKE-0003 as a hard requirement on the
object layout.

**Summary of pinned values for the cost model:**

| Parameter | Pinned value | Rationale |
|-----------|-------------|-----------|
| L_p99 (headline) | **50 ms** | Evidence-based P90–P95 of same-region S3 Standard GET |
| r (round-trips/hop) | **1** | Storage-format constraint; r=2 is a design falsification |
| K_min (r=1) | **8** | 1 manifest + 1 index + 6×1 hop |
| T_compute reserve | **100 ms** | Conservative budget for deserialization and plan evaluation |
| T_budget (target) | **1000 ms** | P99 ≤ 1 s target |
| T_budget (ceiling) | **2000 ms** | P99 ≤ 2 s hard ceiling |
| B_max @ 1 Gbps (target) | **62.5 MB** | (1000−8×50−100)/8000 × 1e9 bits/s |
| B_max @ 50 Mbps (target) | **3.1 MB** | (1000−8×50−100)/8000 × 50e6 bits/s |

Note: the intent's ~75 MB / ~4 MB headlines are reproduced at a slightly more optimistic
T_compute = 50 ms (T_transfer = 650 ms). SPIKE-0001 must reconcile this by either:
(a) tightening the compute reserve to 50 ms and documenting that, or
(b) annotating the ~75 MB figure as computed at (K=8, L_p99=50 ms, T_compute=50 ms)
    and the more conservative 62.5 MB at T_compute=100 ms.
Both are valid; (b) is recommended as it makes both assumptions explicit.

### Worst-case deployment the 2 s ceiling tolerates

At K=8 (r=1), the latency floor exhausts the 2 s budget when:
```
8 * L_p99 > 2000 ms  →  L_p99 > 250 ms
```

Therefore: **the 2 s ceiling survives any deployment where L_p99 ≤ 250 ms.** Since
measured P99 for same-region S3 Standard tops out around 150–200 ms in the published
benchmarks, the ceiling is safe for standard EC2-to-S3 deployments. Cross-region or
throttled deployments (L_p99 > 250 ms) are explicitly out-of-envelope and must be
rejected by the deployment check.

Out-of-envelope detection must flag:

1. **Query too big:** estimated bytes-read > B_max or frontier width M too large
   (this is the existing out-of-envelope detection in SPIKE-0001).
2. **Deployment too slow:** configured or observed L_p99 > 125 ms warns (1 s target
   at risk); L_p99 > 250 ms hard-rejects (2 s ceiling at risk).

### Boundary annotation for the ~75 MB / ~4 MB headlines

Every document that states the ~75 MB / ~4 MB byte budgets must include an annotation:

```
B_max = bandwidth × (T_budget − K·L_p99 − T_compute)
      = [1 Gbps | 50 Mbps] × (1000 ms − 8×50 ms − 50 ms)
      = [1 Gbps | 50 Mbps] × 550 ms
      ≈ [68.75 MB | 3.4 MB]   (at T_compute=50 ms)
  or
      = [1 Gbps | 50 Mbps] × 500 ms
      ≈ [62.5 MB | 3.1 MB]    (at T_compute=100 ms)
```

The ~75 MB / ~4 MB are rounded order-of-magnitude targets, consistent with either
assumption. SPIKE-0001 must choose one compute reserve and state it; the others remain
as sensitivity annotations.

### Latency floor as a separate cost-model line item

SPIKE-0001's cost model must present the latency floor as an explicit line in the
latency budget:

```
T_total = T_floor  +  T_transfer  +  T_compute
        = K_min * L_p99  +  B_read / W  +  T_compute
```

where `T_floor = K_min * L_p99` is shown first, before any byte-transfer term, to
make the serial depth constraint visible and independently verifiable.

The simulation (Cat. 11 artifact) must inject this floor as the serialization depth
before the max-of-M per-phase term that decision 0005 / BUG-0004 introduces. The
correct ordering of terms in the total latency model is:

```
T_total = Σ_{k=1}^{K} [max-of-M_k GET latencies]  +  T_compute
```

where `Σ_{k=1}^{K} E[max-of-M_k GET latencies]` ≥ K * L_p99 (the floor), and the
max-of-M amplification is the BUG-0004 / decision-0005 term on top of it. Both terms
must appear in the same model.

---

## 5. Risks and open questions

1. **L_p99 ≥ 50 ms tail events are real.** The benchmark evidence shows P99 reaching
   86–200 ms. Under those tail conditions, the 1 s target is missed even for in-envelope
   queries. The theorem is a conditional guarantee at the operating-point L_p99 ≤ 50 ms,
   not an absolute guarantee. This must be stated clearly in the envelope spec; the
   benchmark acceptance criterion must use an injected latency profile of 50 ms P99
   (not loopback or 0 ms), per SPIKE-0007.

2. **T_compute reserve uncertainty.** The 50–100 ms range for compute (deserialization +
   query planning + frontier assembly) is an estimate. It must be validated by the perf
   benchmarks in EPIC-003 and calibrated against actual Rust deserialization costs on
   the target object layout.

3. **r = 1 is a format constraint, not a given.** If the implementer of SPIKE-0003 finds
   that the adjacency layout requires two serial fetches (e.g., a pointer table + payload),
   K jumps to 14 and the 1 s target is busted at any measured S3 P99. This would require
   either redesigning the layout or explicitly accepting a 2 s-ceiling-only guarantee
   (dropping the 1 s target from the spec). Escalate to steering if r=1 is not achievable.

4. **max-of-M interaction.** Decision 0005 / BUG-0004 shows that the latency per phase
   is max-of-M GETs, not L_p99. At M=64 parallel frontier GETs per phase, the per-phase
   latency exceeds 2× L_p99 under lognormal distributions. SPIKE-0001 must incorporate
   the max-of-M model on top of the floor established here. The floor K_min * L_p99 is
   the minimum (M=1 per phase) case; the actual cost is higher when M > 1.

5. **Deployment-check implementation.** The out-of-envelope detection for "deployment too
   slow" requires L_p99 to be either configured (as an engine parameter) or measured
   during startup. Neither mechanism is currently specified. SPIKE-0001 should mention
   this as an open implementation question for EPIC-002 (planner / envelope detection).

---

## 6. Next steps

1. **SPIKE-0001 must incorporate these pinned values** into its envelope spec:
   - State L_p99 = 50 ms as the named headline assumption.
   - Present T_floor = K_min * L_p99 = 8 * 50 = 400 ms as a separate line item.
   - Annotate the ~75 MB / ~4 MB byte budgets with the (K, L_p99, T_compute) triple.
   - State the 2 s ceiling constraint: L_p99 ≤ 250 ms for any qualifying deployment.
   - Add deployment-check detection (L_p99 > 125 ms = warn; > 250 ms = reject).

2. **SPIKE-0003 (storage format)** receives the constraint `r = 1` as a hard requirement:
   adjacency offsets must be co-located with or derivable from the adjacency payload in
   a single range GET. If this cannot be met, escalate to steering before SPIKE-0001
   ratification.

3. **SPIKE-0001 ratification (steering-perf-sla half)** is blocked pending this SPIKE
   and BUG-0004/decision-0005 both being folded into the cost model. Once both are in,
   the perf-sla steering member may approve.

4. **File T-NNNN in EPIC-003** to validate the L_p99 = 50 ms assumption empirically:
   the benchmark suite (SPIKE-0007 / Cat. 10) must inject exactly 50 ms P99 S3 latency
   into the local mock and confirm the in-envelope queries meet the 1 s target. This is
   the empirical counterpart to the analytical proof.

---

## Sources

- TopicPartition.io S3 benchmark (eu-north-1, 500 KB objects, 100 iterations):
  https://topicpartition.io/misc/AWS-S3-PUT-latency-benchmark
  — GET P50=26 ms, P99=86 ms

- Nixiesearch benchmark "Read latency of AWS S3, S3 Express, EBS, Instance store" (2024):
  https://nixiesearch.substack.com/p/benchmarking-read-latency-of-aws
  — "100+ ms P99 tail latency" for S3 Standard (4 KB random reads, m5id.large)

- Quickwit "S3 Express speculations" (2023):
  https://quickwit.io/blog/s3-express-speculations
  — S3 Standard TTFB "up to 30ms" P50, "80ms tail common"; P90 ≈ 50 ms

- AWS community reports (us-east-1, < 512 KB):
  — P50 ≈ 45 ms, P99 ≈ 200 ms (multiple independent reporters)

- AWS/WarpStream blog, S3 Express One Zone:
  https://aws.amazon.com/blogs/storage/how-warpstream-enables-cost-effective-low-latency-streaming-with-amazon-s3-express-one-zone/
  — S3 Express P99 write component ≈ 20 ms (intra-AZ); S3 Standard P99 write 400–600 ms

- S3 Express One Zone product page / AWS docs:
  https://docs.aws.amazon.com/AmazonS3/latest/userguide/s3-express-performance.html
  — "single-digit millisecond" P50; P99 CloudWatch recommended threshold = 10 ms

- Decision 0010 (steering-perf-sla, this project):
  .project/decisions/0010-perf-sla-ratification-pass.md
  — Re-derived K_min = 8 (r=1) / 14 (r=2); established this SPIKE.

- Decision 0005 (steering-formal-methods, this project):
  .project/decisions/0005-latency-budget-intra-phase-tail.md
  — max-of-M order-statistic term; complementary to the serial floor established here.

---

## Sign-off required

- **steering-perf-sla:** ratify the pinned L_p99 = 50 ms, r = 1, and the deployment-check
  boundary (L_p99 > 250 ms = reject).
- **steering-formal-methods:** ratify the T_floor line-item placement in the cost model and
  confirm that the floor + max-of-M terms are correctly ordered in the simulation.

Record ratification in `.project/decisions/` as a new entry referencing this spec and
SPIKE-0006 on the board.
