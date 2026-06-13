---
id: T-0014
title: Build discrete-event cold-start latency simulation calibrated to S3 distributions
type: task
status: readypriority: P1
assignee:
epic: EPIC-003
deps: [SPIKE-0001, SPIKE-0006]
rubric_refs: [3, 11]
estimate: M
created: T0+0:20
updated: T0+0:20
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
- [ ] Simulation (Rust or Python) models K phases × M parallel GETs with a configurable per-request latency distribution calibrated to published S3 P50/P99 figures.
- [ ] Includes the intra-phase max-of-M order-statistic tail (BUG-0004) and the serial K·L_p99 floor (SPIKE-0006); both terms are visible in the output breakdown.
- [ ] For an in-envelope query (s, B_max, K from SPIKE-0001) the simulated end-to-end P99 ≤ 1 s; output matches the analytical model within a stated tolerance.
- [ ] An out-of-envelope query is shown to exceed the budget (sanity: the sim does not trivially always pass).
- [ ] Artifact committed under `formal/latency-sim/`; cross-referenced from EPIC-003 and the SPIKE-0001 doc.
- [ ] tests added (the sim's own unit tests); coverage not regressed; `./format_code.sh` green
- [ ] docs / ADR updated if the model assumptions change

## Notes / log
Design-before-code: blocked on SPIKE-0001 + SPIKE-0006. This is the Cat. 11
latency-model evidence; it shares the model with Cat. 3.
