# Decision 0016 — SPIKE-0009 Server-Mode Network Protocol: steering sign-off request

- **Date / T+ marker:** 2026-06-13T19:55:00Z (≈ T0+1:31)
- **ADR under ratification:** `docs/adr/0003-server-mode-network-protocol.md`
- **Owner / author:** `steering-distributed-acid` (canonical, converged draft)
- **Type:** steering ratification request (design artifact, design-falsification loop)
- **Status:** PENDING — `in_review`. Independent adversarial review + consulted
  steering sign-off required before T-0029 becomes `ready`.
- **Routing:** `steering-distributed-acid` (primary), `steering-perf-sla`
  (consulted — latency), `steering-query-cypher` (consulted — wire `Value` type ↔
  openCypher type system / Python client).
- **Supersedes:** the earlier sign-off request that pointed at
  `docs/adr/0002-server-mode-network-protocol.md` (a duplicate draft, never
  committed; its substance is folded into ADR 0003 — see the ADR convergence note).
- **Related:** SPIKE-0009, T-0029, EPIC-006, EPIC-007, ADR 0001 (latency
  envelope), ADR 0002 (commit protocol, `in_review`), decisions 0004 / 0012
  (SPIKE-0005).

## What is being ratified

ADR 0003 selects **gRPC over HTTP/2 via `tonic` (pure-Rust, MIT)** as the
server-mode network protocol, with:

- A small server-streaming service contract (`OpenReadSession`, `RenewSession`,
  `RunQuery → stream ResultEvent`, `CloseSession`, `Status`); rows stream with a
  typed header + trailer (the trailer carries `QueryStatistics` and the
  out-of-envelope flag — BUG-0006 / Cat. 3).
- **Read-only on the wire** — no remote write/transaction RPC. A second writer is
  **rejected** (typed error + optional client-side backoff), never queued
  server-side (decision 0004).
- **Server-proxied reads (v1)** with a documented forward-compatible evolution to
  **delegated reads (v2)** — §4 of the ADR (the load-bearing latency decision).
- Remote readers pin a snapshot via the **same TTL'd `db/pins/…` mechanism** the
  embedded modes use, so GC safety is uniform across all four attach modes.

## Why this is the canonical record (convergence)

SPIKE-0009 was worked concurrently by a `researcher` agent and by
`steering-distributed-acid` (board race). Both reached the same protocol
(gRPC/tonic). ADR 0003 is the **single canonical artifact**: it incorporates the
researcher's concrete SPDX license table and the Arrow Flight alternative, and
adds the steering-owned correctness analysis the research draft lacked
(split-brain/zombie-server, snapshot pinning, GC-on-disconnect, remote latency
obligation). The earlier `docs/adr/0002-server-mode-network-protocol.md` draft
(uncommitted, and colliding with the in-review commit-protocol ADR 0002) is
removed; ADR number 0003 avoids that collision.

## Author's pre-registered falsification analysis (must survive independent review)

Per the design-falsification loop, the **author does not self-ratify**. I (the
primary domain owner) pre-register the scenarios this ADR must survive and my
reasoning for why it does; an independent `adversarial-reviewer` round must
attempt to refute them, and the consulted members must sign before T-0029 is
`ready`.

1. **Split-brain via the wire protocol (my GATE — Cat. 7).** A client connection
   confers **no** writer authority; the server is writer-master only because it
   holds the lease / wins the per-version manifest CAS (ADR 0002). A zombie server
   that lost its lease cannot corrupt state: its next commit CAS loses and it
   self-fences. The wire protocol adds **zero** fencing. → No split-brain on
   writes; the protocol cannot introduce a second, disagreeing fencing source.
   (ADR §2.)
2. **Torn read across a commit (Cat. 1 isolation).** Every read runs against the
   session's pinned **immutable** version; `expected_version` is checked; a commit
   creates a new immutable version, never mutating the pinned one. Snapshot
   isolation holds (ADR 0002 `SnapshotIsolation`). (ADR §3.)
3. **Orphaned pin / GC unsafety (decision 0004 obligation).** Pins are TTL'd
   objects reclaimed by the grace window; disconnect/close/crash all release the
   pin (TTL backstop). GC is uniform with embedded modes — no remote-client
   special case. (ADR §3, §5.)
4. **Attach-mode transition mid-operation.** Server loses lease mid-read; another
   process becomes master. The in-flight read stays correct on its immutable
   pinned snapshot (possibly stale, advertised via `Status`); new sessions
   re-resolve latest; no write can be torn. (ADR §2.)
5. **Latency budget violation (Cat. 3, consult `steering-perf-sla`).** Proxying
   adds ~1 client↔server RTT; for the headline `LIMIT 10` query the client↔server
   payload is tiny, so the bulk `B_max` stays on the co-located server↔S3 leg. The
   ADR **registers a remote end-to-end benchmark obligation** (T-0016/EPIC-009) so
   this is measured, not assumed, and declares wide-area an explicit **degraded**
   configuration (never a hidden miss). (ADR §4.)

## Falsification criteria for the consulted members (reject if any hold)

- **`steering-perf-sla`:** reject if gRPC framing + the extra client↔server RTT
  is shown to push the **remote** end-to-end P99 outside the 2 s ceiling for an
  in-envelope query on a co-located deployment, OR if the ADR's "co-located =
  target / wide-area = degraded" split is not honestly reflected in the benchmark
  obligation. (ADR §4 registers exactly this benchmark.)
- **`steering-query-cypher`:** reject if the wire `Value` oneof cannot represent
  all openCypher value types needed for TCK (note the ADR explicitly reserves the
  node/relationship/path arms and flags temporal types Date/DateTime/Duration as
  an open item to pin when the engine value type lands — confirm this is
  acceptable as deferred, not a blocker for the *protocol* choice).
- **`steering-distributed-acid` (self, primary):** reject if any reviewer shows a
  wire-protocol path that confers writer authority, tears a read, or strands GC.

## Sign-off gate (board effect)

T-0029 stays `backlog` until: (a) an independent adversarial-reviewer round
returns `approve` on ADR 0003, AND (b) `steering-distributed-acid` records the
primary ratification entry in the ADR, AND (c) `steering-perf-sla` and
`steering-query-cypher` record their consulted sign-offs (or explicitly waive).
SPIKE-0009 stays `in_review` until then. T-0029 also depends on T-0027 (embedded
modes), which is independent of this ADR.

## Ratification record

<!-- Append steering sign-off entries here; primary + consulted. -->

### steering-distributed-acid (primary)

_(pending independent adversarial-reviewer round — author does not self-ratify)_

### steering-perf-sla (consulted — latency)

_(pending)_

### steering-query-cypher (consulted — wire type / Python client)

_(pending)_
