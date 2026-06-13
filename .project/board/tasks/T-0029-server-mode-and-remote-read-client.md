---
id: T-0029
title: Server mode (writer-master + serves reads) and a remote read-only client
type: task
status: backlog
priority: P1
assignee:
epic: EPIC-006
deps: [SPIKE-0009, T-0027]
rubric_refs: [7]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

The fourth attach mode: a server process holds the writer role and serves read
queries to remote clients over the protocol chosen in SPIKE-0009. At least one
remote read-only client must query successfully. Design-gated on SPIKE-0009
(protocol ADR) and the embedded modes (T-0027). See `EPIC-006`.

## Acceptance criteria
- [ ] Server process holds the writer lease and exposes a query endpoint over the SPIKE-0009 protocol.
- [ ] A remote read-only client connects and runs a query, receiving correct results (tested end-to-end against the mock-backed server).
- [ ] Server serves concurrent reads to multiple remote clients while it is the writer-master.
- [ ] Graceful handling of client disconnect and server shutdown (no orphaned lease / split-brain).
- [ ] tests added (integration: server + ≥1 remote client); coverage not regressed
- [ ] docs / ADR updated with the server/client usage
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: blocked on SPIKE-0009 + T-0027. Completes the four Cat. 7
attach modes. The Python client (EPIC-007) can target this same protocol.
