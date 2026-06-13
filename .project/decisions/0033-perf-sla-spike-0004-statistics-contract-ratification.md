# Decision 0033 — steering-perf-sla ratification of SPIKE-0004 (manifest statistics contract)

- **Date / marker:** 2026-06-13 (T0+~3:30)
- **Author / role:** `steering-perf-sla` (primary owner of Cat. 3 — latency
  selectivity-envelope definition, byte budgets, phase-bound K, benchmarks,
  out-of-envelope detection)
- **Type:** steering ratification (design-falsification loop). Cross-cutting
  artifact; this record closes the **≥3-of-5 quorum**.
- **Verdict:** **RATIFIED-WITH-CONDITIONS.** Spec status → `accepted`; SPIKE-0004 → `done`.
- **Status:** accepted
- **Artifact ratified:** `docs/specs/SPIKE-0004-manifest-statistics-contract.md`
- **Board item:** `SPIKE-0004` (in_review → done)
- **Rubric:** Cat. 3 (latency envelope + SLA, GATE, w14), Cat. 4 (planner, GATE,
  w12), Cat. 5 (index selectivity, w7); touches Cat. 2 (manifest, GATE, w12).
- **Sign-off request this answers:** decision 0030.
- **Binds / discharges:** decision 0009 (planner stats + tail fan-out), decision
  0015 / ADR 0001 finding **F2** (per-rel-type **max** out-degree mandatory), ADR
  0001 §4.1/§4.3 (O(plan-size), zero extra data-plane round-trip,
  snapshot-consistent), ADR 0001 condition PS-1 (K_min phase-count discipline).

## Mandate applied

My non-negotiable: guard the latency theorem end-to-end and reject anything that
can produce a **silent SLA miss** — "fast only with luck / only when warm". For a
*statistics contract* the relevant failure is an **estimator that under-rejects**:
a query the planner classifies in-envelope that actually busts B_max. I did not
trust the spec's prose. I independently re-derived every load-bearing figure
(deterministic Python, the spec's numbers used only as the claim under test) and
ran eight targeted falsification attacks against the contract's soundness.

## Falsification result: the contract SURVIVES (no escalation)

The contract cannot produce an optimistic accept on any path I could construct. It
discharges every acceptance-criterion bullet of SPIKE-0004 and every binding
condition from ADR 0001 F2 / decision 0015 / decision 0009.

### Independent arithmetic — matches the spec on every load-bearing figure

| Spec claim | perf-sla re-derived | Match |
|------------|---------------------|:-----:|
| B_max(50 Mbps) = 2.88 MB (ADR 0001 design point) | 2.875 MB | ✓ |
| Super-hub: max_deg=4.0e7 × 64 B = 2.56 GB single adjacency list | 2.560 GB | ✓ |
| Super-hub ratio to B_max ≈ 890× | 890.4× | ✓ |
| p99 acceptance est (N_seed=5000, F_tail=120) ≈ 1.65 MB | 1.6527 MB | ✓ |
| p99 estimate ≤ B_max ⇒ "looks in-envelope" (the trap) | True | ✓ |
| F2 byte-bust table (1e5→6.40 MB, 1e6→64 MB, 1e8→6400 MB) | 6.40 / 64.0 / 6400 MB | ✓ |
| Part 5 size: per-(label,property) stored ≈ 1.8 KB | 1804 B | ✓ |
| Part 5 size: inline OOE-critical scalars (L=50,R=30) ≈ 1.0 KB | 1016 B | ✓ |
| Inline scalars ≤ 4 KB manifest reserve (ADR §2.2) | 1016 ≤ 4096 ✓ | ✓ |
| Part 5 size: referenced selectivity blobs ≈ 360 KB | 360.8 KB | ✓ |

The super-hub worked example (spec §3.2) is exactly right: the p99 acceptance
estimate (1.65 MB) passes the byte budget — the **trap** — while the `max_deg`
safety gate catches the 2.56 GB single adjacency list (890× B_max) and classifies
the query out-of-envelope. This is the precise silent-SLA-miss decision 0015
demanded be caught at plan time, and it is caught **only** because `max_deg` is in
the contract. F2 is discharged.

### Falsification attacks (all survived)

- **A1 — super-hub estimator arithmetic.** Re-derived; the two-term estimator
  (p99 for the typical total-byte acceptance estimate; `max_deg` for the
  single-GET super-hub safety gate) correctly classifies the celebrity node OOE
  while the p99-only estimate would have admitted it. Confirms the F2 split.
- **A2 — manifest-bloat / phase-1 inflation.** The always-inline OOE-critical
  scalars are ~1.0 KB, under the cost model's 4 KB `bytes_manifest` reserve. The
  super-hub and non-selective rejection paths run on the phase-1 manifest GET with
  **zero** extra reads. The manifest GET does not inflate the K_min=8 floor.
- **A3 — hidden serial round-trip (the lazy `db/stats` blob fetch).** This is the
  one place a statistics spec could smuggle a serial round-trip onto the cold path.
  The spec (Part 2.1) classifies the referenced-blob fetch as an O(plan-size)
  *planning-phase* lookup "off the K-phase data path." I verified the safety net:
  the blob is needed only to refine selectivity **downward** (to *accept* a query
  the inline `s=1` default would reject) — it is on the accept path, never the
  reject path. Even in the **worst case** where the blob fetch is treated as a full
  9th serial phase, the envelope still closes (usable = 405 ms, B_max(50 Mbps) =
  2.53 MB > 0), exactly matching ADR 0001's PS-1 K=9 fallback. So this does not
  falsify the theorem — but the spec asserts "off the K-phase path" without
  binding it to the phase-count accounting. **→ Condition C1 (below).**
- **A4 — missing/stale rule optimistic-accept leak.** Enumerated every leg of the
  Part 3.3 table. Every leg over-rejects: absent selectivity ⇒ s=1 ⇒ over-rejects;
  absent `max_deg` ⇒ ∞ ⇒ always rejects; absent p99 ⇒ falls back to max_deg ⇒
  inflates; stale ⇒ incremental upper bound (sound). **No optimistic-accept leg
  exists.** The only escape is an explicit, warning-emitting override
  (`SET envelope_check=WARN`, `ALLOW_MISSING_STATS`) — never silent.
- **A5 — incremental `max_deg` soundness under deletion.** After a deletion that
  lowers the true max below the stored value, the stored `max_deg` is
  conservatively **high** ⇒ the super-hub gate fires more readily ⇒ over-rejects.
  Never under-rejects. Sound. (`ANALYZE` tightens it; the over-reject window is the
  safe direction — spec R2.)
- **A6 — freshly-ingested asymmetry.** No-stats ⇒ reject/warn + "run `ANALYZE`".
  Rejecting a maybe-in-envelope query is acceptable; accepting a maybe-SLA-busting
  query is forbidden. The asymmetry points the safe direction.
- **A7 — reachability hole (spec §3.2 note / R5).** The super-hub gate rejects if
  the executor "cannot prove the hub is unreachable." Default is reject; the
  reachability-stats relaxation is explicitly an out-of-scope optimization, not a
  correctness requirement. No optimistic-accept path.
- **A8 — compounding moderate-degree across 6 hops.** A rel-type with
  `max_deg = 40000` *passes* the single-GET super-hub gate (40000 × 64 B = 2.56 MB
  ≤ B_max) yet a 6-hop × M_max=8 expansion could realize ~123 MB (43× B_max) if its
  p99 were also high. This is **not** a falsification: the **total**-byte bound is
  OOE-2's `est_B_query ≤ B_max` (which uses p99 and rejects a high-p99 query), and
  the realized over-read is the hard backstop of the running B_max byte/LIMIT
  counter + early-abort (ADR 0001 F2(c) / Part 5 #5, bound to T-0015/SPIKE-0003).
  The contract correctly separates the two jobs (typical-total vs. single-GET
  super-hub) and the realized counter bounds total bytes regardless. It does,
  however, show the single-GET cap `C_get = B_max` is loose — a `max_deg` just
  under B_max passes the gate while able to consume the whole budget in one GET.
  **→ Non-blocking note PS-A8 (below).**

### License + data guardrails (mandatory check)

- All four named estimator crates are dual-permissive — `hyperloglogplus`
  (Apache-2.0 OR MIT), `probabilistic-collections` (Apache-2.0 OR MIT), `tdigest`
  (Apache-2.0 OR MIT), `blake3` (CC0-1.0 OR Apache-2.0 OR Apache-2.0-WLLVM). No
  GPL/AGPL/SSPL/BUSL or distribution-restricted dependency is required. Clean per
  guardrails §5 (final `cargo deny check licenses` at add-time; AND/OR precedence
  per BUG-0008 — all OR-conjunctions ⇒ permissive). In-tree implementation
  preferred (minimal surface).
- **Data guardrail (§3):** the manifest stores fixed-width **value digests, never
  raw property values** (spec §1.2) — user data is kept out of the manifest by
  construction. See PS-A8b below for the one truncated-order-preserving-key caveat.

## Why ratified-with-conditions, not reject

I attacked the contract on the four axes my Cat. 3 mandate owns — estimator
optimism (can it under-reject?), the cold-start phase-count (does any stats read
add a hidden serial round-trip?), both-bandwidth coverage (the binding 50 Mbps
case), and cache independence (the contract is structurally cache-independent — it
reads only the manifest a cold reader already resolves, no warm-cache assumption
anywhere) — and it survived all four. Every condition below tightens *detection
discipline* or *measurability*; none moves the feasible region or the central
inequality, and each lands naturally on a dependent task that already owns it.

## Conditions (binding on dependent tasks, NOT on this ratification)

- **C1 (→ T-0015 + T-0009/SPIKE-0003, concurs with ADR 0001 PS-1).** The lazy
  `db/stats/<hash>.stats` selectivity-blob fetch (spec Part 2.1 (B) / ADR 0006
  §5.3) MUST NOT silently become a serial data-plane round-trip on the cold
  critical path. Either (a) the blob fetch overlaps phase 1/2 and is provably off
  the K-phase floor (the spec's stated intent), or (b) if any deployment serializes
  it, the phase count is declared K_min=9 and the B_max / OOE-2 / OOE-4 thresholds
  are re-pinned at K=9 (the envelope still closes: usable=405 ms, B_max(50 Mbps)=
  2.53 MB) before T-0015/T-0016 consume them. This is the same phase-count
  discipline as PS-1; T-0015's OOE estimator and T-0016's benchmark must measure
  against whichever K is true.
- **C2 (→ T-0015, re-affirming F1).** The OOE-2 byte estimate and OOE-4 deployment
  thresholds T-0015 implements MUST use the α-corrected forms already bound by F1
  (102 ms / 216 ms) and the two-term estimator from this contract (p99 for the
  total-byte acceptance estimate; `max_deg` for the single-GET super-hub safety
  gate; conservative reject on any frontier whose `max_deg` band exceeds the
  per-GET byte cap). p99 alone as the byte safety bound is forbidden (F2).
- **C3 (→ T-0016, re-affirming PS-2).** Any Cat. 3 measured-SLA evidence that
  exercises OOE detection MUST come from a cold-start, cache-OFF run per the
  cold-start-benchmark-protocol ADR (named profile, fresh state per sample, N≥200).
  A green obtained with the cache enabled or under fast/loopback S3 is not
  acceptable evidence for this envelope.

## Non-blocking notes

- **PS-A8 (→ T-0015 tuning).** The single-GET super-hub cap `C_get = B_max` (spec
  §3.2) is loose: a `max_deg` just under B_max passes the gate while able to consume
  the entire byte budget in one GET. The realized running byte/LIMIT counter is the
  hard backstop, so this is a tuning observation, not a correctness gap. T-0015 may
  set a tighter per-GET cap (e.g. `B_max / (expected concurrent GETs)`) to reject
  earlier; benchmarks (T-0016) can calibrate. Not a ratification blocker.
- **PS-A8b (→ T-0009, guardrails §3).** The histogram stores an order-preserving
  *truncated* key for range interpolation (spec §1.2). T-0009 must truncate coarsely
  enough that the stored key cannot constitute PII/user-data. The manifest never
  lands in the public repo (it lives on S3), and committed fixtures use authored
  tiny data, so this is a low-risk implementer caveat, not a blocker.
- **PS-A8c (R3).** Composite/correlated-predicate selectivity uses independence by
  default, which over-estimates seed size (conservative for OOE). Multi-column stats
  are a future EPIC-005 extension; the conservative direction is safe. Noted.

## Quorum and what unblocks

Cross-cutting artifact ⇒ majority (≥3 of 5). Recorded substantive ratifying
positions on this contract:

1. **`steering-storage`** — decision 0032 / ADR 0006 §5.3: ratified the manifest
   home and the SPIKE-0004 R1 inline-vs-referenced cut (OOE-critical scalars inline;
   `db/stats/<hash>.stats` referenced blob; value-digest privacy), and made the
   binding invariant storage law ("super-hub / non-selective rejection needs no
   data-plane GET beyond the manifest"). The R1 storage call decision 0030 asked of
   storage is made. **Signature 1.**
2. **`steering-formal-methods`** — decision 0015 (secondary owner of Cat. 3/11):
   established F2 as a binding condition requiring per-rel-type **`max_deg`** (not
   only p99) as the super-hub safety term — the contract's central term. This spec
   discharges F2 verbatim. **Signature 2.**
3. **`steering-perf-sla`** (this record) — Cat. 3 detection mandate: the two-term
   estimator is sound, the missing/stale rule cannot under-reject, the contract is
   cache-independent, and no stats read busts the cold-start phase floor (C1). **Signature 3.**

**= 3-of-5. Quorum complete.** SPIKE-0004 spec → `accepted`; board item → `done`.
`T-0009` (manifest + statistics block) and `T-0015` (planner OOE detection) clear
their `SPIKE-0004` dependency; with conditions C1/C2/C3 attached to the tasks that
already own them.

**Recommended (non-blocking) follow-up:** `steering-query-cypher` (the primary
owner named in decision 0030) is invited to append a counter-signature confirming
the selectivity-derivation (MCV + uniform-remainder + histogram interpolation) is
sufficient for the planner's O(plan-size) seed-set sizing. Its founding finding
(decision 0009, which query-cypher authored) is fully discharged by this contract,
and T-0015/T-0009 already carry the contract's terms with no query-cypher objection
filed, so this is a confirmation, not a gate. Ratification stands on the 3-of-5
recorded above.

## Reproduction

Deterministic Python (no RNG), standalone re-runnable. The B_max(50 Mbps)
re-derivation (2.875 MB), the super-hub example (2.56 GB single adjacency list,
890×, 1.65 MB p99 trap), the F2 byte-bust table (6.40/64.0/6400 MB), the Part 5
size analysis (1804 B per-property, 1016 B inline ≤ 4096 B reserve, 360.8 KB
referenced blobs), the K=9 worst-case envelope closure (usable 405 ms, B_max
2.53 MB), the A8 compounding figure (123 MB / 43×), and the missing/stale
optimistic-accept enumeration (no leg under-rejects) were all computed during this
pass and match the spec.
