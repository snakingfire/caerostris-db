# Decision 0017 — steering-perf-sla ratification of SPIKE-0001 (latency selectivity-envelope ADR)

- **Date:** 2026-06-13 (T0+~01:58)
- **Author / role:** `steering-perf-sla`
- **Type:** steering ratification (design-falsification loop, **primary** sign-off)
- **Status:** decided — **RATIFIED-WITH-CONDITIONS** (perf-sla half; quorum now 2-of-2 complete)
- **Artifact ratified:** `docs/adr/0001-latency-selectivity-envelope.md`
- **Board item:** `SPIKE-0001` → `done`
- **Rubric:** Cat. 3 (latency envelope + SLA, GATE, w14), Cat. 11 (formal artifacts, GATE, w6)
- **Complements:** decision `0010` (my own pre-launch ratification pass — K_min·L_p99 floor,
  r≤1, both-bandwidth coverage, benchmark validity), `0015` (formal-methods secondary sign-off,
  F1/F2), `0005` (max-of-M intra-phase tail / BUG-0004), `0009` (planner stats / tail fan-out),
  SPIKE-0006 (L_p99 / r pin), SPIKE-0007 + `docs/adr/0001-cold-start-benchmark-protocol.md`
  (cold-start measurement protocol).

## Mandate applied

My hardest mandate: guard the latency-theorem invariant end-to-end and reject anything that is
"fast only when warm" or "fast only with lucky layout." If no envelope parameters let in-envelope
queries fit in 1 s, the design is falsified — escalate to the full committee. I did not trust the
headline figures or formal-methods' re-derivation: I re-derived every load-bearing number from
first principles (deterministic Python, no RNG, no reuse of the ADR's outputs except as the claim
under test) and ran four falsification attacks on the axes my mandate owns — benchmark validity,
both-bandwidth coverage, worst-case selectivity/byte budget, and hidden serial latency.

## Falsification result: the theorem CLOSES (no escalation)

A feasible parameter region exists at the design point (r=1 ⇒ K_min=8, L_p99=50 ms, M_max=8 ⇒
α=1.10, T_compute=100 ms):

```
T_lat   = K_min·L_p99·α = 8 × 0.050 × 1.10 = 0.440 s
usable  = T_budget − T_lat − T_compute = 1.000 − 0.440 − 0.100 = 0.460 s
B_max(1 Gbps)  = 125  MB/s × 0.460 s = 57.50 MB
B_max(50 Mbps) =  6.25 MB/s × 0.460 s =  2.875 MB
T_query(boundary, both cases) = 1.000 s  ✓
```

**Independent re-derivation matched the ADR and decision 0015 on every load-bearing figure:**
K_min=8; T_lat=440 ms; usable=460 ms; B_max both bandwidths; boundary T_query=1.000 s (exact)
both cases; 1 Gbps seed bound 224,473 (s≈2.245×10⁻⁴); s_max 50 Mbps ≈ 1.109×10⁻⁵; F1 α-corrected
OOE-4 thresholds 102.27 ms / 215.91 ms; F2 super-hub byte busts (out-degree 1e5 → 6.4 MB,
1e6 → 64 MB, both ≫ 2.88 MB B_max). See the table appended to the ADR Sign-off (Round 2).

The §2.2 inline seed-set counts (11,112 / 10,634 / 10,033 / 5,234) reproduce to within ~0.2 %
under a slightly different `bytes_manifest` subtraction order; they are not load-bearing because
at 50 Mbps the binding constraint is the latency floor, not the seed count, and the order of
magnitude (~10⁴ seeds) and the conclusion are identical (note PS-3).

## Falsification attacks (all survived)

- **A1 — benchmark ↔ cost-model coherence.** The `cold-start-benchmark-protocol` ADR pins the
  Cat. 3 measured bars at `nominal-s3` (20 ms) → P99 ≤ 1 s and `slow-s3` (50 ms) → P99 ≤ 2 s.
  `slow-s3`'s 50 ms is exactly this cost model's design-point L_p99; under the α-aware floor it
  leaves usable=460 ms (B_max 50 Mbps = 2.88 MB) at the 1 s target and 1460 ms (9.12 MB) at the
  2 s ceiling — both feasible. The analytical design point and the empirical ceiling profile are
  mutually consistent; a green at these profiles is valid evidence for *this* envelope.
- **A2 — worst-case in-envelope query.** A query at full design-point B_max closes at exactly
  1.000 s at 50 Mbps (boundary), 0.563 s at 1 Gbps, well inside the 2 s ceiling.
- **A3 — F2 super-hub realized latency (Cat. 3 angle).** Early-abort is a partial read of an
  in-flight GET (no extra round-trip); the running byte/LIMIT counter holds total bytes ≤ B_max
  ⇒ transfer ≤ B_max/W regardless of source node. Realized latency is unaffected; F2 is a
  detection-only (estimator-optimism) concern, confirming formal-methods' classification from
  the perf/SLA side.
- **A4 — hidden serial phase (PS-1, see below).** I attempted to find a 9th serial phase. The
  final-row node-property fetch for the surviving LIMIT-10 rows is the candidate: if the storage
  format does not co-locate filter/return properties with hop-6 adjacency, K_min becomes 9 and
  the floor rises to 495 ms. The envelope still closes at K=9 (50 Mbps B_max ≈ 2.53 MB > 0), so
  this is not a falsification — but the ADR's B_max / OOE thresholds are derived at K=8, so the
  K=8 assumption must be protected by the storage format (condition PS-1).

## Findings → conditions

I adopt formal-methods' **F1** and **F2** as binding (both are squarely my Cat. 3
detection mandate) and add two of my own:

- **F1** (condition on T-0015): α-corrected OOE-4 thresholds **102 ms (1 s) / 216 ms (2 s)**,
  not the ADR's uncorrected 112.5 / 237.5 ms. Confirmed by independent computation.
- **F2** (conditions on SPIKE-0004 + T-0015 + SPIKE-0003): maintain per-rel-type **max**
  out-degree; size the byte safety bound from a hard per-GET byte cap (early-abort) or max
  degree, not p99; conservative-reject super-hub frontiers.
- **PS-1** (condition on SPIKE-0003): the K_min=8 phase count must not silently become 9.
  SPIKE-0003 must either co-locate filter/return node properties with the hop-6 adjacency read
  (or index-probe payload) so K stays 8, OR explicitly declare K_min=9 and re-pin SPIKE-0001's
  B_max / OOE thresholds at K=9 before T-0015/T-0016 consume them.
- **PS-2** (condition on T-0016): Cat. 3 measured evidence for this envelope MUST satisfy the
  cold-start-benchmark-protocol ADR — cache explicitly OFF, fresh state per sample, named
  profile (nominal-s3 / slow-s3), N ≥ 200. Cache-on / loopback / fast-s3 results are NOT
  acceptable evidence and are a reject at benchmark review. (Restated here to lock the two ADRs
  together so the cache-independence non-negotiable cannot drift.)

Non-blocking notes: **PS-3** (§2.2 seed-set rounding wobble, cosmetic); **PS-4** (the envelope
ADR does not itself state cache-independence as a first-class property — it is structurally
cache-independent via the cold-start framing and entirely-from-S3 accounting, and the benchmark
ADR enforces it empirically; a one-line statement in §3 would make the non-negotiable
self-evident); **PS-5** (ADR numbering collision already filed as BUG-0010).

## Why ratified-with-conditions, not reject

I tried to break the envelope on the four axes my mandate owns and it survived all four. F1/F2
tighten detection; PS-1 protects the phase-count assumption; PS-2 locks the measurement to
cache-off. None moves the feasible region or the central inequality. All four conditions land on
downstream tasks that already own them (T-0015, T-0016, SPIKE-0003, SPIKE-0004). Per pace
doctrine (Cat. 3 + Cat. 11 are GATEs, weight 14+6; the run is behind at T+~01:58),
ratify-and-unblock is correct; re-spinning the ADR text to absorb PS-1/PS-4 would hold the whole
dependent queue for no change to the proven feasible region.

## Quorum and what unblocks

Latency-envelope parameters: `steering-perf-sla` (primary, this decision) +
`steering-formal-methods` (secondary, decision 0015). **Quorum = 2-of-2 owner signatures on the
committed ADR. Complete.**

- ADR `docs/adr/0001-latency-selectivity-envelope.md` → `accepted`.
- `SPIKE-0001` → `done`.
- Dependent implementation tasks T-0014, T-0015, T-0016 become eligible to flip `ready`
  (SPIKE-0006 already `done`). Planner/integrator to flip on the next grooming pass.
- SPIKE-0003 (in_progress) and SPIKE-0004 proceed against the Part 5 storage constraints with
  conditions F1/F2/PS-1 attached.

## Conditions carried onto dependent board items

- **T-0015** (planner OOE detection): α-corrected OOE-4 thresholds (102 ms / 216 ms);
  max-degree-based byte safety bound; conservative reject on super-hub frontiers.
- **T-0016** (headline cold-start benchmark): cache OFF, fresh state per sample, named profile,
  N ≥ 200, per the cold-start-benchmark-protocol ADR; cache-on/loopback/fast-s3 → reject.
- **SPIKE-0003** (storage format): early-abort adjacency reads as a hard per-GET byte/row cap;
  co-locate filter/return properties with hop-6 adjacency to keep K_min=8 (or declare K=9 and
  re-pin).
- **SPIKE-0004** (statistics contract): maintain per-rel-type **max** out-degree (not only p99).

## Reproduction

Deterministic Python (no RNG), re-runnable standalone, computed during this ratification pass:
the B_max derivation (both bandwidths), the exact boundary T_query check, the seed-set
feasibility points, the 1 Gbps / 50 Mbps seed and s_max bounds, the F1 α-correction
(102.27 / 215.91 ms), the F2 super-hub byte-bust table, and the four falsification attacks
(benchmark-profile feasibility, worst-case in-envelope query, F2 realized-latency,
K=9 hidden-phase feasibility). Matches decision 0015 and the ADR Sign-off Round 1 table.
