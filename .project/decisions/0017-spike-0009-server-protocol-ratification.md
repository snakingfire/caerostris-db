# Decision 0017 — SPIKE-0009 Server-Mode Network Protocol: steering ratification (primary)

- **Date / T+ marker:** 2026-06-13 (≈ T0+1:40)
- **ADR ratified:** `docs/adr/0003-server-mode-network-protocol.md` (status → `accepted`)
- **Owner / ratifier:** `steering-distributed-acid` (primary, Cat. 7 — attach modes / writer leasing)
- **Type:** steering ratification (design-falsification Loop A); supersedes the
  PENDING status of the sign-off request in decision 0016.
- **Verdict:** **APPROVE / RATIFIED** — scoped to the *protocol-selection decision*;
  carried-forward implementation gate recorded (T-0029 stays `backlog`).
- **Routing (per `steering-committee.md`):** `steering-distributed-acid` primary;
  `steering-perf-sla` + `steering-query-cypher` consulted (their scoped concerns
  are registered as tracked obligations in the ADR — see below).
- **Related:** SPIKE-0009, ADR 0003, decision 0016 (sign-off request), decision
  0004 (#3 fencing; non-blocking notes), decision 0014 (formal-methods SPIKE-0005/
  0002 ratification), SPIKE-0002 / ADR 0002 (commit protocol, `in_review`),
  T-0029, T-0027, T-0016, EPIC-006, EPIC-007, EPIC-009.

## What was ratified

ADR 0003 selects **gRPC over HTTP/2 via `tonic` (pure-Rust, MIT)** as the
server-mode network protocol, **read-only on the wire** (no remote write RPC; a
second writer is rejected, never queued — decision 0004), **server-proxied reads
(v1)** with a documented forward-compatible **delegated-reads (v2)** evolution,
and **remote-reader snapshot pinning via the same TTL'd `db/pins/…` mechanism**
the embedded modes use. The wire protocol adds **no fencing** — split-brain
prevention stays entirely in the commit protocol's per-version create-only-CAS
manifest PUT (ADR 0002). License-clean (MIT/Apache-2.0/BSD; SPDX table in ADR §7).

## Falsification pass (Loop A) — all survived

I did not take the ADR's claims about ADR 0002 on faith. I read the commit-protocol
ADR + TLA+ model on the SPIKE-0002 branch and confirmed every cited mechanism is
real and faithfully represented: TTL'd `db/pins/<uuid>`, create-only-CAS
`db/lease/writer`, per-version `manifest/<V>.json` + `PUT If-None-Match:*` fencing
with advisory `_latest`, the model-checked `SnapshotIsolation` /
`AtMostOneCommitPerVersion` invariants (7406 states, no violations — decision 0014),
and GC grace-window rule (grace > max reader-session/renewal period).

| # | Scenario constructed | Outcome |
|---|----------------------|---------|
| 1 | Split-brain via the wire: stalled server S1 wakes after lease expiry + W2 commit, attempts commit | **Survives** — S1's create-only CAS hits 412, self-fences; client connection confers no writer authority. No second fencing source. |
| 2 | Torn read across a commit: V+1 committed while C1's `RunQuery` runs on pinned V | **Survives** — read is against the immutable pinned version; `expected_version` checked; SI holds. |
| 3 | Orphaned pin / GC unsafety on client disconnect or server crash | **Survives** — pin deleted on disconnect; TTL backstop on server crash; grace window > max session; uniform with embedded modes. |
| 4 | **Two-hop liveness (server-mode-specific, not covered by ADR 0002):** client alive + renewing, but the pin-owning server dies | **Survives** — client's next `RenewSession`/`RunQuery` fails cleanly and reconnects; dead server's pin self-expires; any post-death GC of that version is a clean error on the next call, never a torn/partial read. Clean-failure path, not a correctness hole. |
| 5 | Attach-mode transition mid-operation: server loses lease mid-read; another process becomes master | **Survives** — in-flight read stays correct on the immutable pinned snapshot (stale, advertised via `Status`); new sessions re-resolve latest; no write can be torn. |

## Why ratify now, and what this explicitly does NOT do

- The **protocol-selection decision** is squarely my primary domain (sign-off
  table: "Attach modes / writer leasing → steering-distributed-acid") and is
  structurally sound. It adds zero new fencing/isolation surface — a faithful,
  subordinate wire layer over the commit protocol. That is the property I require.
- The two **consulted** concerns are scoped, already registered as tracked
  obligations in the ADR, and do **not** threaten the protocol *choice*:
  - `steering-perf-sla` (Cat. 3): remote end-to-end latency benchmark
    (client↔server leg included) registered on T-0016/EPIC-009; "co-located =
    target / wide-area = degraded" stated honestly (ADR §4). No latency
    falsification found.
  - `steering-query-cypher` (Cat. 8/4): wire `Value` oneof reserves
    node/relationship/path arms; temporal types (Date/DateTime/Duration)
    explicitly deferred to when the engine value type lands (EPIC-002) — a
    fill-in-the-oneof detail, not a protocol re-selection.
- Per the operating model ("decide toward intent, record why, keep moving / never
  block the board") and given we are behind pace, the GATE-Cat.7 protocol choice
  should not stall on two scoped, non-blocking, already-tracked consulted items. If
  either consulted member later refutes their scoped concern, that is a
  `superseded`-class change (new ADR), not a reason to hold this gate open now.

## Carried-forward implementation gate (binding — prove-before-code)

Ratifying SPIKE-0009 unblocks the **design**, not the **code**. **T-0029 stays
`backlog`** until ALL hold:

1. **ADR 0002 / SPIKE-0002 is ratified by `steering-distributed-acid` (primary)** —
   *currently pending; the only recorded sign-off is the formal-methods secondary
   (decision 0014)* — **and landed on `main`** (artifacts are still on
   `work/SPIKE-0002-…`).
2. **The two-concurrent-`PUT If-None-Match:*` → exactly-one-200 mock-fidelity test
   (decision 0014 C-B) is green in CI.**
3. **T-0027 (embedded modes) is `done`** (T-0029's other dep, independent of this ADR).

This honours the prove-before-code rule I jointly enforce with
`steering-formal-methods`: a wire layer may not ship ahead of the commit protocol
it is subordinate to. SPIKE-0009 → `done`; T-0029 remains gated.

## Note to steering-formal-methods

This ADR introduces no new state or invariant requiring a TLA+ change — it is a
faithful client of the existing commit-protocol model (no new fencing/isolation
surface). The server-proxied pin lifecycle is the same `db/pins/` pin/renew/expire
FSM already in `commit_protocol.tla`. If a future implementation diverges (e.g.
server-side pin-ownership semantics differ from the modelled reader-owned pin),
that is a model↔code drift BUG to file at that time.

## Board effect

- ADR 0003 status → `accepted` (ratification entry recorded in the ADR §Steering
  ratification).
- **SPIKE-0009 → `done`** (design ratified).
- **T-0029 → `backlog` (unchanged)** — gated as above. No implementation task is
  falsely unblocked.

**Signed:** steering-distributed-acid  T+~1:40
