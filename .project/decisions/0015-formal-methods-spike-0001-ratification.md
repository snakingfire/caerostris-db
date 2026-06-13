# Decision 0015 — steering-formal-methods ratification of SPIKE-0001 (latency selectivity-envelope ADR)

- **Date:** 2026-06-13 (T0+~01:28)
- **Author / role:** `steering-formal-methods`
- **Type:** steering ratification (design-falsification loop, secondary sign-off)
- **Status:** decided — **RATIFIED-WITH-CONDITIONS** (formal-methods half)
- **Artifact ratified:** `docs/adr/0001-latency-selectivity-envelope.md`
- **Board item:** `SPIKE-0001` (stays `in_review` pending steering-perf-sla
  counter-signature on the committed ADR)
- **Rubric:** Cat. 3 (latency envelope + SLA, GATE, w14), Cat. 11 (formal artifacts, GATE, w6)
- **Complements:** decision `0005` (max-of-M intra-phase tail, this role),
  `0010` (perf-sla ratification pass), `0009` (planner stats / tail fan-out),
  SPIKE-0006 (L_p99 / r pin), SPIKE-0007 (cold-start benchmark protocol).

## Mandate applied

My hardest mandate: if no set of envelope parameters lets in-envelope queries fit in
1 s, the design is falsified — escalate to the full committee. I did not trust the
headline. I re-derived every arithmetic claim from first principles (deterministic
Python, no RNG) and ran three falsification attacks against the capped-frontier bound.

## Falsification result: the theorem CLOSES (no escalation)

A feasible parameter region exists. At the design point (r=1 ⇒ K_min=8, L_p99=50 ms,
M_max=8 ⇒ α=1.10, T_compute=100 ms):

```
T_lat      = K_min · L_p99 · α = 8 × 0.050 × 1.10 = 0.440 s
usable     = T_budget − T_lat − T_compute = 1.000 − 0.440 − 0.100 = 0.460 s
B_max(1 Gbps)  = 125  MB/s × 0.460 s = 57.5 MB
B_max(50 Mbps) =  6.25 MB/s × 0.460 s =  2.875 MB
T_query(boundary) = T_lat + usable + T_compute = 1.000 s   (both bandwidth cases) ✓
```

The 50 Mbps "1.001 s" in the ADR is a rounding artifact of carrying `B_max` as 2.88 MB;
because `usable` is *defined* as `T_budget − T_lat − T_compute`, the exact boundary is
1.000 s by construction. The central inequality is internally consistent.

**Independent re-derivation matched the ADR on all of:** B_max (both bandwidths),
T_lat, both boundary T_query values, the four (F_tail, N_seed) feasibility points, the
1 Gbps seed bound (224,473; s ≈ 2.24×10⁻⁴), and s_max (1.111×10⁻⁵). See the table in
the ADR Sign-off section.

## Findings

### F1 — α dropped from §1.4 ceiling sensitivity and OOE-4 thresholds (condition)

Part 3 uses `T_lat = K_min·L_p99·α`, but §1.4 ("2 s ceiling survives to L_p99 = 237 ms")
and OOE-4 (thresholds 112.5 / 237.5 ms) omit α. α-corrected, self-consistent values:

```
2 s ceiling survives to L_p99 = (2.000 − 0.100) / (8 × 1.10) = 0.216 s  (216 ms, not 237)
OOE-4 1 s warning  threshold  = (1.000 − 0.100) / (8 × 1.10) = 0.102 s  (102 ms, not 112.5)
OOE-4 2 s ceiling  threshold  = (2.000 − 0.100) / (8 × 1.10) = 0.216 s  (216 ms, not 237.5)
```

As written, OOE-4 admits a deployment at L_p99 = 230 ms that the ADR's own α-aware cost
model shows busts the 2 s ceiling — the error points the optimistic (dangerous)
direction. **Condition:** T-0015 implements the α-corrected OOE-4 form.

### F2 — §2.2 byte inequality uses p99 F_tail as a hard cap; super-hub falsifies it (condition)

`bytes_per_hop_phase ≤ M_max · F_tail · bytes_edge_row` treats `F_tail` as a per-node
maximum, but Part 4.1 defines `est_F_tail` as the *p99* out-degree — ~1 % of nodes
exceed it. In a 1B-node power-law graph max out-degree reaches 10⁶–10⁸; one
un-truncated adjacency GET over such a node is 64 MB–6.4 GB and alone busts B_max.

```
out-degree 1e5 → one adjacency list = 6.40 MB   > 2.88 MB  (busts 50 Mbps B_max)
out-degree 1e6 → one adjacency list = 64.0 MB    >> B_max
out-degree 1e8 → one adjacency list = 6,400 MB   >>> B_max
```

The *realized* SLA is protected by the mandated early-abort read (Part 5 #5) acting as
a hard per-GET byte cap + the running LIMIT/byte counter, so the engine does not
over-read. The gap is in *detection*: OOE-2 sizes bytes from the p99 and would admit a
query routed through a super-hub. **Conditions:** (a) §2.2's safety bound must be
stated against a hard per-GET byte cap or the per-rel-type **max** degree, not p99;
(b) SPIKE-0004 must maintain per-rel-type **max** out-degree (not only p99); (c) T-0015
must conservatively reject frontiers whose degree band exceeds the cap, per the same
"optimistic/missing statistic ⇒ reject" doctrine as OOE-5 / decision 0009.

### F3 — ADR numbering collision (non-blocking)

`docs/adr/0001-*` is occupied by both this ADR and `0001-cold-start-benchmark-protocol.md`
(SPIKE-0007). Docs hygiene only. Filed `BUG-0010` for the docs-curator.

## Why ratified-with-conditions, not reject

F1 and F2 do not move the feasible region — they tighten the *classifier* (the boundary
of "in-envelope") and the *estimator's* safety margin so neither can be optimistic. The
proof that a correctly-classified in-envelope query hits the SLA is untouched. Both
conditions land naturally in T-0015 (planner detection) and SPIKE-0004 (statistics
contract), which are downstream of this ADR. Re-spinning the ADR text to hold the whole
dependent queue (SPIKE-0003 in_progress; SPIKE-0004; T-0014/15/16) would cost more than
binding the conditions to the tasks that already own them. Pace doctrine (both affected
categories are GATEs; the run is behind at T+1:22) reinforces ratify-and-unblock over
re-loop.

## Quorum and what unblocks

Latency-envelope parameters: `steering-perf-sla` (primary) + `steering-formal-methods`
(secondary). This decision records the **formal-methods half**. `steering-perf-sla`
substantively pre-approved the framing in decision `0010` and every binding finding from
0010 is folded into the committed ADR — but the two-signature quorum rule requires
perf-sla to append a counter-signature to **this committed ADR**, not only to the
pre-launch decision.

- **Until perf-sla counter-signs the committed ADR:** ADR stays `proposed`; SPIKE-0001
  stays `in_review`; dependent implementation tasks (T-0014, T-0015, T-0016) do **not**
  flip to `ready`. This is the honest two-signature quorum.
- **On perf-sla counter-signature:** ADR → `accepted`; SPIKE-0001 → `done`; T-0014 and
  T-0015 deps clear (SPIKE-0006 already `done`); SPIKE-0003 (in_progress) and SPIKE-0004
  proceed against the Part 5 constraints with conditions F1/F2 attached.

**Request to `steering-perf-sla`:** review F1 (α-corrected OOE-4 thresholds — confirm
they belong to your Cat. 3 detection mandate) and F2 (max-degree statistic), then append
your counter-signature to the ADR Sign-off section. If you concur, this completes
ratification and the integrator/planner can flip the dependent tasks.

## Conditions carried onto dependent board items

- **T-0015** (planner OOE detection): α-corrected OOE-4 thresholds (102 ms / 216 ms);
  max-degree-based byte safety bound; conservative reject on super-hub frontiers.
- **SPIKE-0004** (statistics contract): maintain per-rel-type **max** out-degree in the
  manifest, in addition to p99/tail.
- **SPIKE-0003** (storage format): early-abort adjacency reads as a hard per-GET
  byte/row cap (already Part 5 #5; restated as a ratification condition).

## Reproduction

Deterministic Python (no RNG), re-runnable standalone; the B_max derivation, the
boundary T_query check, the seed-set feasibility table, the α-correction for
§1.4/OOE-4, and the super-hub byte-bust table are all reproduced verbatim in the ADR
Sign-off section and were computed during this ratification pass.
