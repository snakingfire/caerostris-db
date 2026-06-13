---
id: T-0026
title: Implement writer lease + fencing token on the object store
type: task
status: backlog
priority: P1
assignee:
epic: EPIC-006
deps: [SPIKE-0002, SPIKE-0005, T-0010]
rubric_refs: [7, 1]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

The single-writer constraint must be enforced so two processes never both hold the
writer role (split-brain). Per SPIKE-0005, the lease is a liveness aid; safety is
the CAS-on-manifest predicate (already in T-0010). This task implements lease
acquisition/heartbeat/expiry and the fencing token carried into the commit
predicate, handling lease expiry and writer crash/takeover. Design-gated on
SPIKE-0002 + SPIKE-0005. See `EPIC-006`, `EPIC-004`.

## Acceptance criteria
- [ ] Lease acquire/heartbeat/expiry implemented on the object store using the primitive named in SPIKE-0002's ADR.
- [ ] Split-brain prevention: a second process attempting to acquire a held lease is rejected (reject-not-queue, per decision 0004) — tested.
- [ ] Zombie-writer safety: a stalled writer whose lease expired and was taken over by W2 cannot commit stale data — its swap is rejected by the CAS predicate (ties to T-0010) — tested.
- [ ] Lease takeover after a crashed writer leaves the DB consistent (latest committed manifest is the truth).
- [ ] tests added (unit + integration on the mock; concurrency tests); coverage not regressed
- [ ] docs / ADR updated; TLA+ fencing invariant cross-referenced
- [ ] `./format_code.sh` green

## Notes / log
Design-before-code: blocked on SPIKE-0002 + SPIKE-0005. Building block for all
attach modes (T-0027, T-0028, T-0029).
