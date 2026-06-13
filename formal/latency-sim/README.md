# Cold-start latency simulation (`formal/latency-sim/`)

> **Board item:** [T-0014](../../.project/board/tasks/T-0014-discrete-event-latency-simulation.md)
> **Rubric:** Cat. 3 (latency envelope + measured SLA, GATE, w14) and Cat. 11
> (formal verification artifacts, GATE, w6).
> **Proves:** the discrete-event half of the latency theorem — that every query
> inside the selectivity envelope hits **P99 ≤ 1 s cold start** (target) /
> **≤ 2 s** (hard ceiling), **with the cache off**, corroborating the analytical
> cost model ratified in [ADR-0001](../../docs/adr/0001-latency-selectivity-envelope.md).

This is the **simulation** required by
[`docs/process/formal-verification-policy.md`](../../docs/process/formal-verification-policy.md)
Artifact 2 ("Latency cost model + discrete-event simulation"). It is a
self-contained Rust crate with **zero external dependencies** (its own seeded
SplitMix64 RNG and lognormal sampler), so it runs in CI with no network and adds
nothing to the engine's dependency graph.

## Run it

```bash
# Full report (4 scenarios, 100k trials each). Exit code is non-zero if any
# in-envelope SLA assertion fails or the out-of-envelope sanity case does not
# bust the budget — so this binary doubles as a CI check.
cargo run --manifest-path formal/latency-sim/Cargo.toml --release

# Tune trial count / seed:
cargo run --manifest-path formal/latency-sim/Cargo.toml --release -- --trials 200000 --seed 7

# Run the test suite (unit + integration; deterministic, fast):
cargo test --manifest-path formal/latency-sim/Cargo.toml
```

## The model

For one cold-start query trial:

```
T_total = T_lat + T_transfer + T_compute

  T_lat      = Σ_{k=1..K} max(L_{k,1}, …, L_{k,M_k})   // serial across phases,
                                                       // max-of-M within a phase
  T_transfer = B_query / W
  T_compute  = fixed budget (100 ms)
```

* **`K` — serial phase depth.** `K_min = 1 (manifest/version pin) + 1 (index
  probe) + 6 (hops at r=1) = 8`. The phases are *strictly serial*: hop `k+1`'s
  frontier is unknown until hop `k`'s adjacency reads return, so no parallelism
  removes the floor. (SPIKE-0006 / decision 0010.)
* **`max-of-M` per phase.** Each phase issues `M_k` **parallel** range-GETs and
  completes at the **maximum** of their latencies — the intra-phase
  order-statistic tail from **BUG-0004 / decision 0005**. The bare serial floor
  `K · L_p99` is the `M_k = 1` case; `max-of-M` amplifies it. Both terms are
  reported distinctly (`serial floor` vs `lat term P99`).
* **Per-GET latency** is **lognormal**, fitted from a `(P50, P99)` pair. The
  ADR-0001 design point is `P50 = 20 ms`, `P99 = 50 ms`; the decision-0005
  α-calibration distribution is `P50 = 20 ms`, `P99 = 100 ms`.
* **`T_transfer = B_query / W`**, with `B_query ≤ B_max` for an in-envelope
  query. `B_max = W · (T_budget − K·L_p99·α − T_compute)` (ADR §1.7).
* **No cache term.** Every byte comes from S3; the simulation is structurally
  cache-independent, matching the non-negotiable invariant that the cold-start
  SLA holds with the cache off.

Ratified design-point parameters (ADR-0001 §1.1, §1.7):

| Symbol | Value | Source |
|--------|------:|--------|
| `K_min` (r=1) | 8 | SPIKE-0006 |
| `L_p99` | 50 ms | ADR §1.1 |
| `M_max` | 8 | ADR §1.7 |
| `α(M_max=8)` | 1.10 | ADR §1.7 / decision 0005 |
| `T_compute` | 100 ms | ADR §1.6 |
| `T_target` / `T_ceiling` | 1000 ms / 2000 ms | ADR §1.1 |
| `B_max` @ 1 Gbps | 57.5 MB | ADR §1.7 |
| `B_max` @ 50 Mbps (binding) | 2.88 MB | ADR §1.7 |

## Results (seed=1, 100 000 trials)

GET distribution = lognormal P50=20 ms / P99=50 ms (ADR §3.4 design point):

| Scenario | Bandwidth | Sim P99 | Analytic P99 | Δ | Target ≤1 s | Ceiling ≤2 s |
|----------|-----------|--------:|-------------:|----:|:-----------:|:------------:|
| in-envelope headline | 1 Gbps | **889 ms** | 1000 ms | 11% | ✅ | ✅ |
| in-envelope headline | 50 Mbps (binding) | **889 ms** | 1000 ms | 11% | ✅ | ✅ |
| out-of-envelope (50× B_max) | 50 Mbps | 73 429 ms | 73 540 ms | 0.2% | ❌ (correct) | ❌ (correct) |
| slow deployment (L_p99=150 ms) | 1 Gbps | 1545 ms | 1880 ms | 18% | ❌ (correct) | ✅ |

**Reading the numbers.** The in-envelope sim P99 (889 ms) sits *under* the
analytical boundary (1000 ms) with margin. The reason is a deliberate
conservatism in the analytical model: `α = 1.10` was calibrated against the
*worse* `P99 = 100 ms` distribution (decision 0005), but the ADR design-point GET
distribution is the *tighter* `P99 = 50 ms`, whose max-of-M tail is smaller. So
the analytical reserve (`T_lat = 440 ms`) over-estimates the realised latency term
(sim `≈ 329 ms`), and in-envelope queries clear the SLA with headroom — exactly
the safe direction for a GATE. The sim and the analytical model agree within the
stated **15% tolerance** for both in-envelope bandwidth cases.

The **out-of-envelope** and **slow-deployment** rows confirm the sim does *not*
trivially always pass (AC4): an unselective filter (50× the byte budget) blows
past the ceiling on the transfer term, and a slow deployment (`L_p99 = 150 ms`)
busts the 1 s target on the *serial floor alone* (`8 × 150 = 1200 ms`),
independent of bytes — corroborating SPIKE-0006's "deployment too slow"
out-of-envelope condition.

## Calibration cross-check (reproduces decision 0005)

Run at the calibration distribution (lognormal P50=20 ms / P99=100 ms), the
latency-only probe `Σ_{K} max-of-M` reproduces the steering-formal-methods
Monte-Carlo table from
[decision 0005](../../.project/decisions/0005-latency-budget-intra-phase-tail.md)
to within ~1.5%, independently confirming the α factors the ADR relies on:

| K | M | decision-0005 P99 (ratio) | this sim P99 (ratio) |
|--:|--:|--------------------------:|---------------------:|
| 3 | 1   | 193 ms (0.64) | 190.3 ms (0.63) |
| 3 | 8   | 332 ms (1.11) | 338.3 ms (1.13) |
| 3 | 64  | 527 ms (1.76) | 529.5 ms (1.77) |
| 3 | 256 | 693 ms (2.31) | 694.2 ms (2.31) |
| 5 | 8   | 491 ms (0.98) | 489.7 ms (0.98) |

This agreement is what justifies using `α(M_max=8) = 1.10` in the analytical
cost model, and is encoded as a regression test
(`reproduces_decision_0005_max_of_m_amplification` in `tests/envelope.rs`).

## Calibration against the mock (future work)

Per the formal-verification policy, once the storage implementation exists this
simulation must be **re-calibrated against measured latency** from the local S3
mock (MinIO/moto + injected latency, per
[`docs/process/testing-and-benchmarks.md`](../../docs/process/testing-and-benchmarks.md) §7
and [ADR 0001-cold-start-benchmark-protocol](../../docs/adr/0001-cold-start-benchmark-protocol.md)).
The simulated and measured P99 must agree within tolerance; the measured-SLA half
of Cat. 3 is the headline benchmark task **T-0016**. This artifact is the
**analytical/simulation** half — the empirical benchmark must independently
confirm the same envelope, cache OFF, fresh state per sample, N ≥ 200.

## Layout

```
formal/latency-sim/
  Cargo.toml          # standalone, zero-dependency crate (own [workspace])
  README.md           # this file
  src/
    lib.rs            # the model: lognormal sampler, phase model, percentiles,
                      #   analytic cross-check; with unit tests
    main.rs           # CLI: runs the 4 scenarios, prints the report + verdict
  tests/
    envelope.rs       # integration tests encoding the T-0014 acceptance criteria
```
