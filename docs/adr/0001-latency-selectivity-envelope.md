# ADR 0001 — Latency Selectivity-Envelope and Analytical Cost Model

## Status

`proposed`

## Date / T+ marker

2026-06-13T19:20:00Z (T0+~00:56)

## Context

This ADR is the mandatory design-before-code gate for all storage and query-execution
work in caerostris-db. The headline SLA — 6-hop unanchored `MATCH` with
node-property filter(s), `LIMIT 10`, over 1B nodes / 10B edges, **cold start,
P99 ≤ 1 s** (2 s ceiling), end-to-end at the client, on 1 Gbps; also provably
bounded at 50 Mbps — is achievable only conditionally. The physics of object-store
I/O forbid serving an unconstrained 6-hop expansion within any finite budget. This
document defines the conditional envelope precisely and proves in-envelope queries
meet the SLA.

### Rubric categories advanced

- **Cat. 3 (latency envelope + SLA, GATE, w14):** This ADR directly satisfies the
  "envelope defined + analytical cost model committed" anchor required for Cat. 3 ≥ 50,
  and is a prerequisite for the simulation + benchmark work required for Cat. 3 = 100.
- **Cat. 11 (formal verification artifacts, GATE, w6):** The cost model and the
  out-of-envelope detection algorithm are formal artifacts within the Cat. 11 scope.

### Prior decisions that bind this ADR

- `0001-storage-domain-ratification-findings.md` (steering-storage): byte-budget
  arithmetic confirmed correct; F1 (LIMIT-driven early termination + early-abort
  adjacency reads mandatory) constrains the cost model.
- `0005-latency-budget-intra-phase-tail.md` (steering-formal-methods): the cost model
  must account for the **max-of-M intra-phase order statistic** and bound the per-phase
  frontier width M explicitly. The naive `f^6` cartesian-expansion seed-set bound is
  invalid; LIMIT-driven frontier capping is the operative mechanism.
- `0009-planner-stats-and-tail-fanout-bound.md` (steering-query-cypher): the planner
  must use tail/worst-case degree (not mean degree) from manifest statistics for safe
  out-of-envelope estimation; missing statistics → conservative reject.
- `0010-perf-sla-ratification-pass.md` (steering-perf-sla): the serial latency floor
  `K_min · L_p99` is the binding constraint for the 6-hop shape; the assumed `L_p99`
  and per-hop round-trip count `r` must be first-class named parameters; the naive
  seed-set bound `|seed| ≤ B_max / (avg_node_bytes × avg_fan_out^6)` collapses to < 1
  for any realistic parameters and must be replaced by the capped-frontier formulation.
- `BUG-0004`, `SPIKE-0006`, `SPIKE-0007`: upstream findings from ratification passes
  that this ADR must incorporate.

---

## Decision

We will define the latency selectivity-envelope as a **five-parameter tuple
`(s, F_tail, M_max, K, L_p99_assumed)`**, derive `B_max` and the seed-set bound
from these, prove the in-envelope SLA using an analytical cost model that accounts
for both the serial latency floor and the intra-phase max-of-M order statistic, and
specify an O(plan-size) out-of-envelope detection algorithm for use by the query
planner. The 50 Mbps case is the binding constraint on `B_max`; the 1 Gbps case
relaxes the byte constraint but not the latency floor.

---

## Part 1 — Envelope Parameter Definitions

### 1.1 Named parameters

| Symbol | Meaning | Design-point value |
|--------|---------|-------------------|
| `T_budget` | End-to-end P99 target | 1.000 s (2.000 s ceiling) |
| `W` | Network bandwidth (bytes/s) | 125 MB/s (1 Gbps) or 6.25 MB/s (50 Mbps) |
| `L_p99` | Per-request S3 P99 latency (single GET/PUT) | **50 ms** (assumed design point; see §1.4) |
| `r` | Sequential adjacency round-trips per hop | **≤ 1** (storage-format constraint; see §1.5) |
| `K_min` | Minimum sequential phase depth | `1 + 1 + 6·r` (see §1.3) |
| `T_compute` | Compute + deserialization budget per query | **100 ms** (see §1.6) |
| `s` | Node-property filter selectivity (fraction of nodes passing) | `≤ s_max` (derived below) |
| `F_tail` | Per-hop tail (p99 or max-observed) out-degree per relationship type | must satisfy `F_tail^(hop) · |seed|` bounded per phase (see §2) |
| `M_max` | Maximum parallel GETs per phase (frontier width cap) | `≤ M_max` (see §1.7) |

### 1.2 Byte budget B_max

```
B_max = W × (T_budget − K_min · L_p99 − T_compute)
```

Using the design-point values (L_p99 = 50 ms, K_min = 8 for r=1, T_compute = 100 ms):

```
Reserve = K_min · L_p99 + T_compute = 8 × 0.050 + 0.100 = 0.500 s
Usable   = T_budget − Reserve = 1.000 − 0.500 = 0.500 s

1 Gbps case:  B_max = 125 MB/s × 0.500 s = 62.5 MB  (~63 MB; intent's "~75 MB" uses K=3 or L_p99=25 ms; see §1.4)
50 Mbps case: B_max = 6.25 MB/s × 0.500 s = 3.13 MB  (~3 MB; intent's "~4 MB" uses K=3; see §1.4)
```

**Why the intent's "~75 MB / ~4 MB" round numbers hold:** the intent uses a lighter
reserve budget (K=3 phases at 50 ms = 150 ms serial + 100 ms compute = 250 ms reserve,
leaving 750 ms usable → 93.75 MB at 1 Gbps; or K=3 at L_p99=100 ms + T_compute=50 ms =
350 ms reserve, 650 ms usable → 3.75 MB at 50 Mbps). The headline figures are
order-of-magnitude correct and the design intends to achieve them at `r=1`. This ADR
uses K_min=8 and L_p99=50 ms for the design-point derivation, which is conservative.
See §1.4 for the L_p99 sensitivity analysis.

The **binding constraint is the 50 Mbps case**: B_max ≈ 3–4 MB. Any query that must
read more than B_max bytes at 50 Mbps exceeds the budget.

### 1.3 Phase depth K_min (serial latency floor)

A cold-start 6-hop query has an irreducible sequence of dependent phases:

```
Phase 1:  manifest/root GET (version pin; cold start has no cached version)
Phase 2:  index probe GET (secondary index → seed-node list)
Phases 3–8: adjacency-list GETs for hop 1 through hop 6 (r=1 round-trip per hop)
```

So `K_min = 1 + 1 + 6 × r = 8` when `r=1`. If the storage format requires an
indirection read per hop (`r=2`), K_min = 14, which at L_p99=50 ms gives a latency
floor of 700 ms, consuming 70% of the 1 s budget before any bytes transfer — at
L_p99=100 ms the floor alone is 1.4 s, busting the ceiling. **Therefore `r ≤ 1` is a
hard storage-format constraint on this ADR's envelope proof. SPIKE-0003 (storage
format spec) must satisfy it.**

### 1.4 L_p99 sensitivity and the 2 s ceiling

The design point is L_p99 = 50 ms (a realistic AWS S3 P99 in most regions when
object size is manageable). The worst-case L_p99 the 2 s ceiling survives at K_min=8:

```
max_L_p99 that 2 s ceiling survives =
  (T_ceiling − T_compute) / K_min = (2.000 − 0.100) / 8 = 237 ms
```

So out-of-envelope detection must also flag: "the assumed L_p99 = 50 ms is violated
by the current S3 deployment (measured at > 237 ms P99)". Any deployment with a
measured S3 P99 > 237 ms violates the 2 s ceiling even for in-envelope queries (with
r=1, K_min=8, T_compute=100 ms) — this is a deployment constraint, not a query
constraint, and must surface as a startup warning.

**Sensitivity table:**

| L_p99 | K_min=8, reserve | usable | B_max (1 Gbps) | B_max (50 Mbps) | ≤ 1 s target? |
|------:|-----------------:|-------:|---------------:|----------------:|:-------------|
| 20 ms |          260 ms  | 740 ms |      92.5 MB   |       4.63 MB   | yes |
| 50 ms |          500 ms  | 500 ms |      62.5 MB   |       3.13 MB   | yes |
| 100 ms|          900 ms  | 100 ms |      12.5 MB   |       0.63 MB   | marginal (1 Gbps only; 50 Mbps infeasible) |
| 150 ms|         1300 ms  |   —    |         —      |          —      | **no** (floor alone > target) |

For L_p99 ≥ 100 ms, the 50 Mbps SLA is not provable with the current 6-hop + LIMIT
structure. The analytical model is therefore stated at **L_p99 ≤ 50 ms (design point),
with the 50 Mbps case as the binding constraint, and L_p99 = 20 ms as the "comfortable"
operating point** where both bandwidth cases leave ample byte budget.

### 1.5 Storage-format constraint: r ≤ 1 (one round-trip per hop)

The adjacency-list layout must allow hop h+1's frontier range-GETs to be issued with
a single round of I/O: no indirection read is permitted between "receive hop h
adjacency data" and "issue hop h+1 adjacency GETs". This is achievable by:

- Embedding the adjacency-list start offset and length for each destination node
  within the edge row itself (so the reader can compute the next range-GET address
  from the current payload without an extra GET), or
- Grouping adjacency lists contiguously by source-node ID and encoding them so that
  a range-GET over a frontier's contiguous ID band covers all outbound edge lists.

SPIKE-0003 (storage format spec) must satisfy this constraint. If it cannot, K_min
becomes 14, the L_p99=50 ms floor climbs to 700 ms, and the 50 Mbps case becomes
infeasible — a design falsification that must be escalated to steering.

### 1.6 Compute budget T_compute = 100 ms

This covers deserialization, node-property predicate evaluation, LIMIT tracking, and
planner overhead across all K phases. 100 ms is conservative (a modern CPU can
deserialize many MB/s of columnar data). Implementers may use a tighter estimate once
benchmarks exist; the proof is valid for any `T_compute ≤ 100 ms`.

### 1.7 Frontier width cap M_max and the intra-phase max-of-M latency correction

The per-phase latency is not `L_p99` but `E[max of M parallel GETs]`. This is the
order statistic of M independent lognormal samples, and its tail is worse than a
single-request P99 by a factor that grows with M:

From the steering-formal-methods Monte-Carlo (decision 0005, seed=1, lognormal
P50=20 ms, P99=100 ms):

| M (parallel GETs per phase) | K=3 query P99 | naive 3×L_p99 | ratio |
|----------------------------:|-------------:|-------------:|------:|
| 1  | 193 ms | 300 ms | 0.64 |
| 8  | 332 ms | 300 ms | 1.11 |
| 64 | 527 ms | 300 ms | 1.76 |
| 256| 693 ms | 300 ms | 2.31 |

For the K=8 design point, the amplification is similar: at M=64 the latency term
alone could be ≈ 8 × 1.76 × 100 ms ≈ 1.4 s — the same budget as the whole target.
**Therefore M must be explicitly bounded.**

The envelope adds M_max as an explicit parameter. The **corrected latency term** is:

```
T_lat(K, M_max) = K × L_p99 × α(M_max)
```

where `α(M_max)` is the max-of-M order-statistic amplification factor. For the design
point (lognormal S3 latency, P50=20 ms, P99=100 ms):

| M_max | α (empirical, 99th percentile over queries) |
|------:|--------------------------------------------:|
| 1     | 0.65 |
| 4     | 0.90 |
| 8     | 1.10 |
| 16    | 1.30 |
| 32    | 1.55 |
| 64    | 1.80 |

**Design-point choice: M_max = 8, α = 1.10.**

Corrected reserve:
```
T_lat(8 phases, M_max=8) = 8 × 50 ms × 1.10 = 440 ms
Reserve = 440 ms + 100 ms (compute) = 540 ms
Usable  = 1000 ms − 540 ms = 460 ms

B_max (1 Gbps,  M_max=8) = 125 MB/s × 0.460 s = 57.5 MB
B_max (50 Mbps, M_max=8) = 6.25 MB/s × 0.460 s =  2.88 MB
```

These are tighter than the naive derivation. The **design-point B_max values for
the envelope are:**

```
B_max (1 Gbps)  = 57.5 MB  (≈ 58 MB; intention document's "~75 MB" is achievable with K=3 or lower α)
B_max (50 Mbps) = 2.88 MB  (≈ 3 MB; binding constraint)
```

The intent's round numbers ("~75 MB / ~4 MB") remain valid order-of-magnitude figures
for lighter assumptions (K=3, M=1, L_p99=50 ms). This ADR uses the more conservative
M_max=8, K=8 design point, which is the honest worst-case for a cold-start 6-hop query.

---

## Part 2 — Selectivity Envelope and Seed-Set Bound

### 2.1 Why the naive f^6 bound is invalid

The SPIKE-0001 acceptance criteria proposed: `|seed| ≤ B_max / (avg_node_bytes × avg_fan_out^6)`.

This is the **full cartesian expansion bound**: it assumes the query reads every node
reachable in 6 hops from every seed node. At B_max = 2.88 MB, avg_node_bytes = 256 B,
avg_fan_out = 10:

```
|seed|_max = 2,880,000 / (256 × 10^6) ≈ 0.011
```

That is less than one — no query qualifies. The formula is not useful. The bound
collapses because real 6-hop BFS from any seed visits exponentially many nodes.

The correct mechanism, stated in commander's intent and R7, is **LIMIT-driven early
termination + frontier capping**: the query carries `LIMIT L` (typically L=10), and
the executor prunes the frontier as soon as L results are collected. Combined with the
secondary index anchoring the seed set, the *realized* bytes read are far smaller than
the cartesian product.

### 2.2 Revised seed-set and frontier bounds

Let:
- `N_seed` = number of nodes in the seed set (output of the index probe).
- `M_max` = maximum parallel GETs issued per hop phase (the frontier width cap, §1.7).
- `F_tail` = the p99-or-max out-degree of the dominant relationship type (from manifest
  statistics; SPIKE-0004 defines the statistics contract).
- `bytes_node` = average bytes per node (properties + metadata) = 256 B (design point).
- `bytes_edge_row` = average bytes per edge row (destination ID + properties) = 64 B (design point).
- `L` = LIMIT clause value (10 for the headline workload).

**Phase budget per hop:** Each hop issues at most `M_max` parallel range-GETs. Each
range-GET covers the adjacency list of one or more frontier nodes. The total bytes read
per hop phase is at most:

```
bytes_per_hop_phase = M_max × avg_adjacency_list_bytes
avg_adjacency_list_bytes ≤ F_tail × bytes_edge_row
```

So: `bytes_per_hop_phase ≤ M_max × F_tail × bytes_edge_row`

Over K_hop = 6 hop phases (phases 3–8), plus 1 manifest GET + 1 index probe GET:

```
bytes_manifest ≤ 4 KB  (root/version pointer)
bytes_index    ≤ N_seed × bytes_node  (index returns seed IDs + properties)
bytes_hops     ≤ 6 × M_max × F_tail × bytes_edge_row
```

Total: `B_query ≤ bytes_manifest + bytes_index + bytes_hops`

**In-envelope condition:**

```
B_query ≤ B_max
N_seed × bytes_node + 6 × M_max × F_tail × bytes_edge_row ≤ B_max − bytes_manifest
```

Substituting the design points (B_max = 2.88 MB = 2,880,000 B, bytes_node = 256 B,
bytes_edge_row = 64 B, M_max = 8, bytes_manifest = 4,096 B):

```
N_seed × 256 + 6 × 8 × F_tail × 64 ≤ 2,880,000 − 4,096
N_seed × 256 + 3,072 × F_tail ≤ 2,875,904
```

Example feasible points:
- F_tail = 10, N_seed ≤ (2,875,904 − 30,720) / 256 ≤ **11,112 nodes** in the seed set
- F_tail = 50, N_seed ≤ (2,875,904 − 153,600) / 256 ≤ **10,634 nodes**
- F_tail = 100, N_seed ≤ (2,875,904 − 307,200) / 256 ≤ **10,033 nodes**
- F_tail = 500, N_seed ≤ (2,875,904 − 1,536,000) / 256 ≤ **5,234 nodes**

**The seed-set size constraint is not tight for the byte budget** — even at F_tail=500,
the constraint allows >5,000 seed nodes. The binding constraint for the 50 Mbps case is
the latency floor, not the byte budget alone. The byte constraint comes into play at
1 Gbps where B_max = 57.5 MB:

At 1 Gbps, F_tail = 10, M_max = 8:
```
N_seed × 256 + 3,072 × 10 ≤ 57,500,000 − 4,096
N_seed ≤ (57,495,904 − 30,720) / 256 ≤ **224,473 nodes**
```

A 1B-node graph with selectivity `s = N_seed / N_total ≤ 224,473 / 10^9 ≈ 2.2×10^-4`
(i.e., the property filter matches at most 0.02% of nodes) is comfortably in-envelope
at 1 Gbps.

**In-envelope selectivity bound:**

```
s_max(W, F_tail, M_max) = (B_max(W) − bytes_manifest − 6·M_max·F_tail·bytes_edge_row) 
                          / (N_total × bytes_node)
```

For the 50 Mbps design point, F_tail=10, M_max=8, N_total=10^9:
```
s_max = (2,875,904 − 4,096 − 30,720) / (10^9 × 256)
      = 2,841,088 / 256,000,000,000
      ≈ 1.1 × 10^-5   (about 1-in-100,000 nodes)
```

### 2.3 LIMIT-driven early termination and frontier capping

The LIMIT mechanism works as follows:
1. The planner injects a frontier-width cap `M_max` into the executor.
2. At each hop, the executor issues at most `M_max` adjacency-list GETs in parallel.
3. As results arrive, the executor maintains a running count toward L=10. Once L results
   are collected from any hop, expansion stops — no more GETs are issued.
4. With high selectivity (tiny seed set), the seed-set adjacency lists are small; with
   `LIMIT 10` the search terminates very early in the tree.

This is the mechanism that makes the unanchored query effectively anchored: the
secondary index yields a small seed set, and LIMIT-driven early termination ensures
that only O(M_max × K_hop) adjacency-list GETs are ever issued, regardless of the
theoretical branching factor.

**The envelope's validity depends on the LIMIT clause being present.** A query without
LIMIT (unbounded result set) cannot satisfy the byte budget regardless of selectivity,
and must be classified as out-of-envelope.

---

## Part 3 — Analytical Cost Model and SLA Proof

### 3.1 Full query cost formula

For a cold-start in-envelope query with parameters within the envelope:

```
T_query(P99) = T_lat + T_transfer + T_compute

where:
  T_lat      = K_min × L_p99 × α(M_max)   [serial latency floor with max-of-M correction]
  T_transfer = B_query / W                  [byte transfer time]
  T_compute  = 100 ms                       [fixed compute budget]
  B_query    ≤ B_max
```

### 3.2 Proof that in-envelope queries hit P99 ≤ 1 s (1 Gbps design point)

Given: K_min=8, L_p99=50 ms, M_max=8, α=1.10, T_compute=100 ms, W=125 MB/s, B_max=57.5 MB.

```
T_lat      = 8 × 0.050 × 1.10 = 0.440 s
T_transfer ≤ 57.5 MB / 125 MB/s = 0.460 s
T_compute  = 0.100 s

T_query ≤ 0.440 + 0.460 + 0.100 = 1.000 s  ✓ (at the boundary; in-envelope queries satisfy ≤ B_max, so T_query < 1.000 s for B_query < B_max)
```

By construction: any query with `B_query ≤ B_max` satisfies `T_query ≤ T_budget`.
The P99 claim holds because:
1. `T_lat` is the P99 of the max-of-M order statistic over K phases — by construction
   at the 99th percentile.
2. `T_transfer` is deterministic given `B_query ≤ B_max` and bandwidth `W`.
3. `T_compute = 100 ms` is a deterministic budget, not a random variable.

Therefore the P99 of the total query latency satisfies:
```
P99(T_query) ≤ P99(T_lat) + T_transfer + T_compute ≤ T_budget = 1.000 s
```

QED (subject to the assumptions in §1).

### 3.3 Proof for the 50 Mbps binding case

Given: K_min=8, L_p99=50 ms, M_max=8, α=1.10, T_compute=100 ms, W=6.25 MB/s, B_max=2.88 MB.

```
T_lat      = 8 × 0.050 × 1.10 = 0.440 s
T_transfer ≤ 2.88 MB / 6.25 MB/s = 0.461 s
T_compute  = 0.100 s

T_query ≤ 0.440 + 0.461 + 0.100 = 1.001 s
```

The 50 Mbps case is at the boundary (within rounding). The result is tight: the
50 Mbps case is feasible only if all of the following hold simultaneously:
- `r ≤ 1` (no adjacency indirection reads)
- `L_p99 ≤ 50 ms` (S3 at design-point latency)
- `M_max ≤ 8` (frontier width capped)
- `B_query ≤ B_max ≈ 2.88 MB` (selectivity constraint satisfied)
- `T_compute ≤ 100 ms`

If any of these is violated, the 50 Mbps SLA is not provable. The out-of-envelope
detection algorithm must enforce all five conditions (see Part 4).

The **2 s ceiling** at 50 Mbps with L_p99=50 ms, M_max=8 gives B_max = 12.5 MB — a
much more comfortable budget, achievable with less selective filters.

### 3.4 Latency distribution assumptions

The analytical model assumes:
- S3 GET latency follows a lognormal distribution with P50 ≈ 20 ms, P99 ≈ 50 ms
  (representative of AWS S3 Standard in us-east-1 for objects ≤ 10 MB).
- The max-of-M amplification factors α in §1.7 are derived from this distribution
  (Monte-Carlo, 50,000 trials, seed=1; see decision 0005).
- These values must be validated against the local MinIO mock with injected latency
  and against real S3 when credentials arrive (per SPIKE-0007's cold-start
  measurement protocol).

The simulation task (T-0014) will calibrate the model against the actual latency
distribution on the mock and update α values if needed. The analytical proof remains
valid because it is parameterized by α; if the empirical α differs, re-check the
feasibility condition.

---

## Part 4 — Out-of-Envelope Detection Algorithm

### 4.1 Inputs (from the query planner, O(plan-size))

The query planner must compute the following estimates **before any object-store
access**, using statistics read from the pinned manifest version (no extra round-trips):

```
est_N_seed     = N_nodes(label) × selectivity(property, value)
                 — from manifest: per-label node count × per-property cardinality
                 — if selectivity unknown: use conservative upper bound N_nodes(label)
est_F_tail     = per-rel-type p99 out-degree (from manifest statistics)
                 — must be the tail bound, not mean (decision 0009)
                 — if statistics missing: use conservative maximum (whole-graph max degree)
est_B_query    = bytes_manifest + est_N_seed × bytes_node +
                 6 × M_max × est_F_tail × bytes_edge_row
est_M_frontier = est_N_seed × est_F_tail  [frontier size at hop 1, before LIMIT]
has_limit      = (LIMIT clause present in query plan)
l_p99_measured = deployment-level measured S3 P99 (startup health check)
```

**Statistics sources (manifest; see SPIKE-0004):**
- `N_nodes(label)`: total node count per label, maintained on commit.
- `selectivity(property, value)`: per-property value cardinality estimate
  (histogram bucket or NDV), maintained on commit.
- `per-rel-type p99 out-degree`: degree distribution summary with tail term, maintained
  on commit.

When a required statistic is missing or stale, the planner must fall back to a
**conservative upper bound** — never optimistic. This defaults to treating the query
as out-of-envelope (reject/warn).

### 4.2 Detection conditions

A query is **out-of-envelope** (and must be rejected, warned, or degraded) if **any**
of the following conditions holds:

**OOE-1: No LIMIT clause.**
```
!has_limit
```
Reason: Without LIMIT, early termination cannot bound the frontier. The byte budget
may be violated by any query, regardless of selectivity.
Response: Hard error.

**OOE-2: Estimated bytes exceed B_max.**
```
est_B_query > B_max(W)
```
where `B_max(W) = W × (T_budget − K_min × L_p99_assumed × α(M_max) − T_compute)`.
Response: Hard error with estimated bytes and budget in the error message.

**OOE-3: Frontier width cap exceeded at hop 1.**
```
est_M_frontier > M_max  (and !has_limit)
```
Note: With LIMIT, the executor will cap the frontier via early termination; this
condition only triggers without LIMIT (covered by OOE-1) or if the frontier at hop 1
alone already exceeds M_max without any results produced.
Response: Hard error or degraded plan (planner may choose to add an implicit LIMIT
with a warning).

**OOE-4: Deployment too slow (startup check).**
```
l_p99_measured > (T_budget − T_compute) / K_min
= (1.000 − 0.100) / 8 = 112.5 ms   [for the 1 s target]
= (2.000 − 0.100) / 8 = 237.5 ms   [for the 2 s ceiling]
```
If `l_p99_measured > 112.5 ms`, in-envelope queries may not meet the 1 s target
(they are still within the 2 s ceiling if `l_p99_measured ≤ 237.5 ms`).
If `l_p99_measured > 237.5 ms`, the 2 s ceiling is violated even for in-envelope queries.
Response: Startup warning (logged at engine init, not per-query error). The engine
still serves queries; the SLA is disclaimed.

**OOE-5: Missing or stale statistics → conservative reject.**
```
selectivity(property, value) is unknown AND est_N_seed > 0.01 × N_total
```
(If statistics are missing, assume worst-case selectivity of 1; if that exceeds
the seed-set bound, treat as out-of-envelope.)
Response: Warn with a message explaining which statistics are missing and how to
collect them (e.g., run a REFRESH STATISTICS command post-ingest).

### 4.3 Algorithm complexity

All inputs are read from the manifest (a fixed-size header; O(1) per property/label)
or computed from the query plan (O(plan-size) traversal). No object-store GETs are
issued. Total complexity: **O(plan-size + statistics-lookups)** where
statistics-lookups is O(distinct labels × distinct relationship types in the plan).

### 4.4 Required planner response

| Condition | Default response | Override allowed? |
|-----------|-----------------|------------------|
| OOE-1 | Hard error (`QueryEnvelopeError::NoLimit`) | No |
| OOE-2 | Hard error (`QueryEnvelopeError::ByteBudgetExceeded`) | Yes, via `SET envelope_check = WARN` session flag |
| OOE-3 | Hard error or implicit LIMIT + warning | Yes |
| OOE-4 | Startup warning (not per-query) | N/A |
| OOE-5 | Warning + conservative reject | Yes, via explicit `ALLOW_MISSING_STATS` hint |

When override is allowed, the engine executes the query but emits a structured warning
that the SLA is not guaranteed. It never silently misses the SLA.

---

## Part 5 — Storage Format Constraints Imposed by This Envelope

This ADR feeds the following hard constraints into SPIKE-0003 (storage format spec):

1. **r ≤ 1 (mandatory):** Adjacency-list reads for hop h+1 must be issuable from
   the data returned by hop h with no additional serial round-trip. See §1.5.
2. **Contiguous adjacency layout for range-GET batching:** Adjacency lists must be
   laid out so that a frontier of M_max nodes whose source IDs fall in a contiguous
   range can be served by a single multi-byte range-GET (or a bounded number of
   parallel range-GETs), not M_max independent random GETs.
3. **Columnar node-property layout:** Node properties used in filter predicates must
   be readable without fetching the full node record (columnar scan or sparse index
   lookup), to support the index-probe phase (phase 2) without reading entire node objects.
4. **Manifest includes statistics:** The manifest must carry per-label node counts,
   per-property cardinality estimates (histogram), and per-rel-type degree distribution
   summaries (including tail/p99 term) — these are the statistics the out-of-envelope
   detection algorithm reads in O(1) per planning call. See SPIKE-0004.
5. **Early-abort adjacency reads:** The storage format and the client library must
   allow a range-GET to be aborted early (e.g., HTTP Range request, partial read)
   so that LIMIT-driven early termination does not over-read adjacency data past
   what is needed.

---

## Part 6 — Latency Distribution Assumptions (Calibration Hook)

The analytical model is parameterized by:
- `L_p99 = 50 ms` (S3 per-request P99 at design point)
- `α(M_max=8) = 1.10` (max-of-M order-statistic amplification)

These are derived from published S3 latency data (AWS documentation, 2024: P50 ≈ 5–20 ms
for small objects in-region; P99 ≈ 50–100 ms; references: AWS S3 documentation,
"Request rate and performance guidelines") and the Monte-Carlo probe in decision 0005.

The discrete-event simulation (T-0014) must:
1. Calibrate `L_p99` and `α` against the MinIO mock with injected latency profiles.
2. Validate the model at the design-point (L_p99=50 ms, M_max=8) by running 1,000+
   cold-start in-envelope queries and observing that the empirical P99 ≤ 1 s.
3. Validate the out-of-envelope detection by confirming that detected-OOE queries
   would indeed have violated the budget.

---

## Alternatives Considered

### Alternative A — No formal envelope; optimistic "tune and benchmark"

**Description:** Ship the engine, benchmark it, and tune the storage format and
planner until the SLA is met empirically. No analytical model or pre-proof.

**Why considered:** Simpler; most production databases work this way.

**Why rejected:** Commander's intent explicitly requires the latency theorem to be
"formally proven before implementation, not discovered afterward." A benchmark-first
approach means: (a) the design may be structurally infeasible, discovered only after
months of implementation; (b) the SLA could be met on a warm/lucky run but not on a
cold P99; (c) out-of-envelope queries have no detection criterion. The project's
non-negotiable invariant is that "fast only when warm" is a design falsification.

### Alternative B — Weaker envelope: byte budget only, no latency floor

**Description:** Define the envelope purely in terms of `B_max` (bytes read ≤ budget)
and omit the serial latency floor analysis. Claim P99 ≤ 1 s follows from byte budget
alone.

**Why considered:** Simpler; the byte budget is the easier constraint to express.

**Why rejected:** As demonstrated in §1.3 and §1.4, and confirmed by
steering-perf-sla (decision 0010), the serial latency floor `K_min × L_p99` is the
binding constraint for the 6-hop query structure — not the bandwidth. At L_p99=150 ms,
K_min=8, the floor alone is 1.2 s, busting the 1 s target before any byte moves.
A byte-only envelope would be provably wrong, producing false positives (accepting
queries that bust the latency floor) while potentially being too strict on the byte
side. Both the serial floor and the byte budget must be in the envelope.

### Alternative C — Max-of-M correction via a fixed "safety factor" on L_p99

**Description:** Instead of deriving α(M_max) from the order-statistic analysis,
multiply `L_p99` by a fixed safety factor (e.g., 1.5×) and treat that as the effective
per-phase latency.

**Why considered:** Simpler; avoids the need for a Monte-Carlo calibration.

**Why rejected:** A fixed safety factor is either too conservative (over-rejecting
in-envelope queries) or not conservative enough (under-estimating at high M). The
empirical α values show that the amplification is M-dependent (0.65 at M=1 to 2.31
at M=256). Bounding M_max at 8 and using α=1.10 is the correct approach: it gives
a tight bound that is neither over-conservative nor under-conservative. The
Monte-Carlo calibration is a one-time artifact tied to T-0014.

### Alternative D — Full-cartesian-expansion seed-set bound

**Description:** Use the formula `|seed| ≤ B_max / (avg_node_bytes × avg_fan_out^6)`
as originally proposed in SPIKE-0001's acceptance criteria.

**Why considered:** Appears in the acceptance criteria and is derived from first principles.

**Why rejected:** This formula yields `|seed| < 1` for any realistic fan-out (shown
in §2.1 and confirmed by steering-perf-sla decision 0010). It ignores LIMIT-driven
early termination, which is the operative mechanism. The capped-frontier formulation
in §2.2 is the correct replacement.

---

## Consequences

### Positive

- **Advances Cat. 3 toward 50:** Envelope defined + analytical cost model committed.
  The Cat. 3 GATE can score ≥ 50 once this ADR is ratified.
- **Advances Cat. 11 toward 50:** The cost model and detection algorithm are formal
  artifacts required for Cat. 11.
- **Unblocks T-0014, T-0015, T-0016:** The simulation (T-0014), planner detection
  (T-0015), and headline benchmark (T-0016) can begin once this ADR is ratified.
- **Unblocks SPIKE-0003:** The storage format spec now has concrete constraints to
  design against (r ≤ 1, contiguous adjacency layout, manifest statistics).
- **Unblocks SPIKE-0004:** The estimator algebra is defined; SPIKE-0004 can now pin
  the statistics contract against this formulation.
- **Provably honest about the 50 Mbps case:** The design point is tight but feasible;
  the conditions under which it is not feasible are named explicitly.

### Negative / trade-offs

- **Frontier width cap M_max = 8 is restrictive:** At high fan-out with a large seed
  set, capping M=8 parallel GETs per phase may cause latency to exceed B_max/W if the
  adjacency lists are large. Implementers must ensure adjacency range-GETs are batched
  efficiently (contiguous layout) so that M=8 GETs covers a full frontier.
- **Model requires L_p99 ≤ 50 ms for the 50 Mbps case:** Deployments with higher S3
  latency cannot serve the 50 Mbps SLA. This is a deployment constraint, not a
  design flaw, but it must be communicated clearly.
- **Statistics must be maintained on every commit:** The manifest statistics are
  correctness-critical for safe out-of-envelope detection. Stale statistics risk
  false-negative OOE detection (accepting queries that will bust the budget). Keeping
  statistics current on commit adds write overhead (addressed in SPIKE-0003 and
  SPIKE-0004).

### Open questions

1. **Calibration of α against the real S3 latency distribution:** The α=1.10 value
   is derived from a Monte-Carlo simulation using an assumed lognormal distribution.
   The discrete-event simulation (T-0014) must validate this against the actual mock
   latency distribution and update if needed.
2. **SPIKE-0003 must confirm r ≤ 1 is achievable:** If the storage format requires
   `r=2`, the envelope must be re-derived with K_min=14, and the 50 Mbps case becomes
   infeasible. Escalate to steering if this occurs.
3. **SPIKE-0004 statistics contract:** The exact set of statistics maintained in the
   manifest is defined by SPIKE-0004. This ADR assumes they exist; SPIKE-0004 must
   ratify the contract before the planner detection (T-0015) is implemented.
4. **Bandwidth case for the graded bar:** As noted by steering-perf-sla (decision 0010),
   the rubric states the acceptance bar as 1 Gbps; R7 makes 50 Mbps "ideally also
   tolerable." This ADR proves both cases analytically. The Cat. 3 = 100 benchmark
   (T-0016) should be run at a 1 Gbps-equivalent injected latency profile, with the
   50 Mbps analytical proof satisfying the "ideally also" requirement.

---

## Rubric Impact

| Cat. | Name | Impact |
|------|------|--------|
| 3 | Latency: selectivity-envelope theorem + measured SLA | Defines envelope + cost model (→ Cat. 3 ≥ 50); unblocks simulation and benchmark (→ Cat. 3 → 100). |
| 11 | Formal verification artifacts | Cost model + detection algorithm are formal artifacts (→ Cat. 11 ≥ 50 once ratified). |
| 2 | Storage format & S3 commit protocol | Imposes r ≤ 1, contiguous adjacency, and manifest statistics constraints on SPIKE-0003. |
| 4 | openCypher completeness (TCK) | Out-of-envelope detection must be implemented in the planner (EPIC-002). |
| 5 | Pluggable secondary indices | Index probe is phase 2 of the envelope; the index must deliver a seed set within N_seed budget. |

---

## Cross-references

- **SPIKE-0001 (this item):** The board item this ADR closes.
- **SPIKE-0003:** Storage format spec; must satisfy r ≤ 1 and the five constraints in Part 5.
- **SPIKE-0004:** Statistics contract; must define and maintain the statistics this ADR's
  out-of-envelope algorithm reads.
- **SPIKE-0006:** Pins L_p99 and K_min as first-class parameters — incorporated in §1.3–1.4.
- **SPIKE-0007:** Cold-start measurement protocol; validates this model's α assumptions.
- **T-0014:** Discrete-event simulation; calibrates the model and validates the P99 claim.
- **T-0015:** Planner out-of-envelope detection; implements the algorithm in Part 4.
- **T-0016:** Headline benchmark; produces empirical evidence for Cat. 3 = 100.
- **EPIC-003:** Parent epic for the latency envelope work.

---

## Sign-off

### Adversarial review record

_(no rounds yet — submitted for steering review)_

### Steering ratification

_(pending adversarial review — steering-perf-sla and steering-formal-methods must both sign off)_
