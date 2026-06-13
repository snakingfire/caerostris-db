---
id: SPIKE-0009
title: Choose server-mode network protocol (gRPC / custom TCP / HTTP) — ADR
type: spike
status: ready
priority: P2
assignee:
epic: EPIC-006
deps: []
rubric_refs: [7]
estimate: S
created: T0+0:20
updated: T0+0:20
---

## Context

Server mode (Cat. 7 GATE) exposes the query interface to remote read-only clients.
The protocol choice (gRPC via tonic, a custom framed TCP protocol, or HTTP/JSON)
shapes the server and client tasks (T-0029) and must be decided by ADR before that
implementation is `ready`. License-clean, permissive deps only. This is a design
decision, not blocked on any other spike. See `EPIC-006`.

## Acceptance criteria
- [ ] ADR committed to `docs/adr/` comparing ≥2 options on: latency overhead vs. the cold-start budget, streaming-result support, dependency license (permissive only), and Python-client friendliness (EPIC-007).
- [ ] A recommendation is selected with rationale; the wire shape for "run query / stream rows" is sketched.
- [ ] Dependency license check recorded (SPDX) for the chosen stack.
- [ ] Cross-referenced from EPIC-006 (T-0029) and EPIC-007.
- [ ] No implementation code required — ADR only.
- [ ] `./format_code.sh` green (valid markdown)

## Notes / log
Ready now — independent design task. Output unblocks T-0029 (server + remote
client). Keep the protocol thin so it does not eat the latency budget.
