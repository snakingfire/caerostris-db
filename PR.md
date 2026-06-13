# PR: T-0014 — Build discrete-event cold-start latency simulation calibrated to S3 distributions

## Board item

[.project/board/tasks/T-0014-discrete-event-latency-simulation.md](.project/board/tasks/T-0014-discrete-event-latency-simulation.md)

Branch: `work/T-0014-latency-sim-reland` (based on the latest `main`, `105cf9b`).

> **Re-land note.** A prior session authored a complete, ADR-faithful
> implementation on `work/T-0014-cold-start-latency-sim` (commit `963585e`) and
> set this item `in_review`, but that branch never cleared the review gate and was
> left 9 commits behind `main` when its session ended. Rather than duplicate
> ~1.1k lines of correct work, this PR adopts that artifact onto a fresh branch
> off the latest `main` (cherry-picked the artifact commit), re-verifies it green,
> and re-opens it through the adversarial-review + pre-mortem gate.

## Rubric refs

Cat. 3 (latency: selectivity-envelope theorem + measured SLA, GATE, w14) and
Cat. 11 (formal verification artifacts, GATE, w6). This is the **simulation** half
of the Cat. 3 / Cat. 11 latency-model evidence; the *measured* benchmark half is
T-0016.

## Acceptance criteria (from board item)

- [x] Simulation (Rust or Python) models K phases × M parallel GETs with a configurable per-request latency distribution calibrated to published S3 P50/P99 figures. — `formal/latency-sim/src/lib.rs` (`simulate`, `LatencyDist::lognormal_from_p50_p99`).
- [x] Includes the intra-phase max-of-M order-statistic tail (BUG-0004) and the serial K·L_p99 floor (SPIKE-0006); both terms are visible in the output breakdown. — `SimReport.serial_floor_ms` vs `SimReport.lat_term_p99_ms`; test `breakdown_exposes_floor_and_max_of_m_terms`; CLI prints both as distinct line items.
- [x] For an in-envelope query (s, B_max, K from SPIKE-0001) the simulated end-to-end P99 ≤ 1 s; output matches the analytical model within a stated tolerance (15%). — tests `in_envelope_p99_under_one_second_1gbps` / `..._50mbps_binding`; sim 889 ms vs analytic 1000 ms.
- [x] An out-of-envelope query is shown to exceed the budget (sanity: the sim does not trivially always pass). — tests `out_of_envelope_query_busts_the_budget`, `slow_deployment_busts_floor_independent_of_bytes`.
- [x] Artifact committed under `formal/latency-sim/`; cross-referenced from EPIC-003 and the SPIKE-0001 doc (ADR-0001). — EPIC-003 Notes/log + checkbox; ADR-0001 open-question #1.
- [x] tests added (the sim's own unit tests); coverage not regressed; `./format_code.sh` green. — 17 tests (10 unit + 7 integration); engine crate untouched (separate workspace, so the root crate's coverage is unaffected).
- [x] docs / ADR updated if the model assumptions change. — no model assumptions changed; the sim *confirms* ADR-0001's α=1.10; ADR-0001 open-question #1 annotated with the sim result; `formal/latency-sim/README.md` documents the model + results.

## Summary of change

Adds `formal/latency-sim/`, a self-contained, **zero-external-dependency** Rust
crate (its own `[workspace]`) that corroborates the analytical latency cost model
ratified in [ADR-0001](docs/adr/0001-latency-selectivity-envelope.md) by
Monte-Carlo discrete-event simulation. For each cold-start query trial it assembles
`T_total = T_lat + T_transfer + T_compute`, where `T_lat` is the sum over `K = 8`
strictly-serial phases (1 manifest + 1 index probe + 6 hops at r=1) of the
**max-of-M** parallel range-GET latencies — the intra-phase order-statistic tail
from BUG-0004 / decision 0005, layered on the serial `K·L_p99` floor from
SPIKE-0006. Per-GET latency is lognormal fitted from a `(P50, P99)` pair;
randomness comes from a seeded SplitMix64 + Box–Muller normal so every percentile
is reproducible in CI with no network. There is **no cache term** — the simulation
is structurally cache-independent, matching the non-negotiable invariant. The crate
ships a CLI (`cargo run --manifest-path formal/latency-sim/Cargo.toml --release`)
that runs 4 scenarios and exits non-zero on any SLA violation, so it doubles as a
CI check. It is deliberately a separate workspace so it adds nothing to the engine
crate's dependency graph or build.

## Test evidence

`cargo nextest run --manifest-path formal/latency-sim/Cargo.toml`:

```
caerostris-latency-sim         (unit, src/lib.rs)  : 10 passed
caerostris-latency-sim::envelope (tests/envelope.rs): 7 passed
Summary: 17 tests run: 17 passed, 0 skipped
```

`cargo clippy --manifest-path formal/latency-sim/Cargo.toml --all-targets -- -D warnings`: **clean** (Finished, no warnings).

`cargo fmt --manifest-path formal/latency-sim/Cargo.toml --all --check`: **clean** (no diff).

`./format_code.sh` (root engine crate + TOML): **green** — root crate clippy clean,
fmt + taplo applied, no files changed. Engine crate (`cargo test`): green (6 tests +
3 doctests), untouched by this PR.

Simulation report (`cargo run … --release -- --trials 20000 --seed 1`; GET lognormal
P50=20 ms / P99=50 ms; cache OFF; matches the 100k-trial figures in the README to the
millisecond):

| Scenario | Bandwidth | Sim P99 | Analytic P99 | Δ | ≤1 s | ≤2 s |
|----------|-----------|--------:|-------------:|----:|:----:|:----:|
| in-envelope headline | 1 Gbps | **889 ms** | 1000 ms | 11% | YES (correct) | YES |
| in-envelope headline | 50 Mbps (binding) | **889 ms** | 1000 ms | 11% | YES (correct) | YES |
| out-of-envelope (50× B_max) | 50 Mbps | 73 430 ms | 73 540 ms | 0.2% | NO (correct) | NO (correct) |
| slow deployment (L_p99=150 ms) | 1 Gbps | 1544 ms | 1880 ms | 18% | NO (correct) | YES |

CLI verdict: **PASS** (exit 0) — in-envelope queries meet the SLA cold, cache OFF;
out-of-envelope and slow-deployment cases correctly bust the budget.

Calibration cross-check (lognormal P50=20 / P99=100) reproduces the decision-0005
max-of-M α table to within ~1.5% (K=3,M=8 → 338 ms vs 332 ms; K=3,M=256 → 694 ms vs
693 ms), independently confirming α(M_max=8) ≈ 1.10. See
`formal/latency-sim/README.md` for the full table and discussion.

**Note on the 11% gap (intentional, safe direction):** the analytical reserve uses
α=1.10 calibrated against the *worse* P99=100 ms distribution; the ADR §3.4
design-point GET distribution is the *tighter* P99=50 ms, so the realised latency
term (≈329 ms) is below the 440 ms analytical reserve and the in-envelope P99 closes
*under* the 1000 ms boundary with margin. The model is conservative.

**Scope boundary:** this is the analytical/simulation half of Cat. 3. The *measured*
benchmark on the MinIO mock with injected latency (cache OFF, fresh state per sample,
N ≥ 200) is the separate task T-0016 (per ADR-0001 condition PS-2). No engine code
exists yet to benchmark; this artifact does not require it.

## Review gate

- [ ] adversarial-reviewer sign-off (see docs/process/adversarial-review-loops.md)
- [ ] premortem-analyst sign-off (see docs/process/adversarial-review-loops.md)
- [x] `./format_code.sh` green
- [x] `cargo nextest run --manifest-path formal/latency-sim/Cargo.toml` green (17 tests)
- [x] coverage not regressed (new crate fully unit+integration tested; engine crate untouched)
- [x] board item updated to `in_review`

<!-- Reviewers: append your verdict block below this line per adversarial-review-loops.md -->
