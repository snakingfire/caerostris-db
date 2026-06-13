---
id: T-0014
title: Build discrete-event cold-start latency simulation calibrated to S3 distributions
type: task
status: in_review
priority: P1
assignee: implementer-wf_94c471c3-447-13
epic: EPIC-003
deps: [SPIKE-0001, SPIKE-0006]
rubric_refs: [3, 11]
estimate: M
created: T0+0:20
updated: T0+4:10
---

## Context

Cat. 3 = 100 requires a discrete-event simulation that corroborates the SPIKE-0001
analytical cost model: it models K sequential phases, M parallel GETs per phase
with a realistic per-request latency distribution (the intra-phase max-of-M tail
from BUG-0004 / decision 0005, and the serial-depth K·L_p99 floor from SPIKE-0006 /
decision 0010), and reports the end-to-end P99 for in-envelope queries. Output is a
committed artifact under `formal/latency-sim/`. Design-gated on SPIKE-0001 +
SPIKE-0006 ratification. See `EPIC-003`, `docs/process/formal-verification-policy.md`.

## Acceptance criteria
- [x] Simulation (Rust or Python) models K phases × M parallel GETs with a configurable per-request latency distribution calibrated to published S3 P50/P99 figures. — `formal/latency-sim/src/lib.rs` (`simulate`, `LatencyDist::lognormal_from_p50_p99`).
- [x] Includes the intra-phase max-of-M order-statistic tail (BUG-0004) and the serial K·L_p99 floor (SPIKE-0006); both terms are visible in the output breakdown. — `SimReport.serial_floor_ms` vs `SimReport.lat_term_p99_ms`; test `breakdown_exposes_floor_and_max_of_m_terms`.
- [x] For an in-envelope query (s, B_max, K from SPIKE-0001) the simulated end-to-end P99 ≤ 1 s; output matches the analytical model within a stated tolerance. — tests `in_envelope_p99_under_one_second_1gbps` / `..._50mbps_binding`; sim 889 ms vs analytic 1000 ms (≤15%).
- [x] An out-of-envelope query is shown to exceed the budget (sanity: the sim does not trivially always pass). — tests `out_of_envelope_query_busts_the_budget`, `slow_deployment_busts_floor_independent_of_bytes`.
- [x] Artifact committed under `formal/latency-sim/`; cross-referenced from EPIC-003 and the SPIKE-0001 doc (ADR-0001). — EPIC-003 Notes/log + checkbox; ADR-0001 open-question #1.
- [x] tests added (the sim's own unit tests); coverage not regressed; `./format_code.sh` green — 17 tests (10 unit + 7 integration); engine crate untouched (separate workspace).
- [x] docs / ADR updated if the model assumptions change — no model assumptions changed; the sim *confirms* ADR-0001's α=1.10; ADR-0001 open-question #1 annotated; `formal/latency-sim/README.md` documents the model + results.

## Notes / log
Design-before-code: blocked on SPIKE-0001 + SPIKE-0006. This is the Cat. 11
latency-model evidence; it shares the model with Cat. 3.

- **T+2:35 implementer-wf_94c471c3-447-13:** claimed and **re-landing** the
  discrete-event simulation. A prior session had authored a complete, correct
  implementation on `work/T-0014-cold-start-latency-sim` (commit `963585e`) and set
  the item `in_review`, but that branch never cleared the review gate and was left
  9 commits behind `main` when its session ended. Rather than duplicate ~1.1k lines
  of correct, ADR-faithful work, I adopted it onto a fresh branch
  `work/T-0014-latency-sim-reland` off the latest `main` (cherry-picked the artifact
  commit; verified it builds, all 17 tests green under nextest, clippy clean,
  `./format_code.sh` green, CLI verdict PASS). Re-opening through the adversarial
  review + pre-mortem gate.
