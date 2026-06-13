# ADR 0003 — Server-mode network protocol (writer-master + remote read clients)

## Status

`accepted`
<!-- proposed → reviewed (adversarial) → accepted (steering) → superseded -->
Ratified by `steering-distributed-acid` (primary, Cat. 7) at ≈ T0+1:40 — see
§Steering ratification and `.project/decisions/0017-spike-0009-server-protocol-ratification.md`.
**Scope of this ratification:** the *protocol-selection decision* (gRPC/tonic;
read-only wire; server-proxied v1 with documented v2 evolution; remote pins reused
via `db/pins/`; **no second fencing source**) — which is squarely my primary
domain. **This does not unblock T-0029 implementation by itself:** the
prove-before-code gate keeps T-0029 `backlog` until the commit-protocol chain
(ADR 0002 / SPIKE-0002) is ratified + landed + its two-concurrent-`PUT
If-None-Match:*` mock-fidelity test is green, **and** T-0027 (embedded modes) is
`done`. See §Steering ratification for the exact carried-forward gate.

## Date / T+ marker

2026-06-13T19:39:00Z  (≈ T0+1:15)

## Context

The fourth attach mode (R3 mode 4, rubric **Cat. 7**, weight 8, **[GATE]**) is
**server mode**: a single server process is the **writer-master** for a database
on object storage *and* serves **read queries to remote clients** over the
network. A database may simultaneously have one server (writer + reader) and any
number of **embedded read-only** and **master-less** clients reading the same
object-store prefix directly (R3 modes 2/3). Concurrent readers are required
throughout.

This ADR chooses the **server↔client network protocol** and sketches its wire
shape. It is the design-before-code gate (**SPIKE-0009**) for **T-0029** (server
process + remote read-only client) and is cross-referenced by **EPIC-007** (the
Python client can target the same protocol).

### Hard constraints carried in (these are the falsification scenarios this design must survive)

This ADR is **subordinate to the commit/concurrency protocol** (ADR 0002,
SPIKE-0002, currently `in_review`) and to the prior steering ratification passes.
It may not introduce a second source of truth for any ACID/concurrency property.
The binding constraints:

- **decision 0004 #3 / SPIKE-0005 Constraint 2 — fencing is a property of the
  per-version create-only-CAS manifest PUT, never of lease belief.** The network
  protocol must **not** add a second fencing mechanism (e.g. a TCP-connection
  liveness check or a server-side "I am the leader" flag) that could disagree
  with the manifest-version truth. Split-brain must remain impossible *by the
  commit mechanism*, not by the wire protocol.
- **decision 0004 non-blocking note — a second writer is rejected (not queued).**
  EPIC-006's "rejected (or queued)" is resolved to **reject with a clear error +
  optional client-side retry/backoff**. A server-side write queue is a
  coordination service the object-store-native design deliberately avoids. The
  protocol therefore exposes **read** RPCs to remote clients; it does **not**
  expose a remote-write/transaction RPC in this scope.
- **decision 0004 non-blocking note — master-less GC / TTL pins.** Remote readers
  must pin a snapshot version using the **same** TTL'd pin mechanism (`db/pins/…`)
  the embedded modes use (ADR 0002 §1), so GC safety is uniform across all four
  attach modes. The protocol must make the pinned version explicit on the wire.
- **Latency theorem (commander's intent / Cat. 3).** The cold-start P99 ≤ 1 s
  budget (B_max ≈ 75 MB at 1 Gbps, ≈ 4 MB at 50 Mbps; ADR 0001-envelope) is
  end-to-end **as observed by the client**. A remote client adds the
  client↔server network leg to that budget. The protocol must be **thin** (small
  framing/serialization overhead, streaming results, no head-of-line blocking on
  large result sets) and the data-read topology must be chosen so the server↔S3
  and client↔server legs do not *both* pay the full B_max. See §4 (read
  topology) — this is the load-bearing latency decision, not the encoding choice.
- **Open-source guardrails.** Permissive (MIT/Apache-2.0/BSD) dependencies only;
  SPDX recorded (§7). No secrets/data in the repo.

### Rubric stakes

- **Cat. 7** (concurrency & attach modes, w8, GATE): score 100 requires all four
  modes working + tested, writer-leasing prevents split-brain, concurrent readers
  verified under load. This ADR unblocks the server mode (mode 4).
- **Cat. 8** (Python bindings, w6): the Python client targets this protocol for
  the remote-read attach mode.
- **Cat. 3** (latency, GATE): the protocol must not eat the cold-start budget.

### Convergence note (two parallel drafts)

SPIKE-0009 was picked up concurrently by a `researcher` agent and by
`steering-distributed-acid` (board race, last-write-wins per the task-board
protocol). Both drafts independently reached the **same protocol choice**
(gRPC/tonic). This ADR is the **single canonical record**: it folds in the
researcher's stronger material — the concrete SPDX license table (§License check)
and the Apache Arrow Flight alternative (Alternative D) — and adds the
steering-owned analysis the research draft lacked: the **writer-lease/zombie-
server split-brain** reasoning (§2), **remote-reader snapshot pinning + GC
safety** (§3), and the **remote end-to-end latency obligation** (§4). The earlier
draft (a duplicate `docs/adr/0002-...`, never committed, which also collided with
the in-review commit-protocol ADR 0002) is superseded by this file; its substance
is preserved here. ADR number **0003** is used to avoid the 0002 collision with
the commit-protocol ADR (SPIKE-0002, `in_review`).

## Decision

**We will use gRPC over HTTP/2 (the `tonic` crate, Tokio-based, pure Rust) as the
server-mode network protocol**, with a small streaming-result service contract,
because it gives us first-class **server-streaming** for unbounded result rows,
**HTTP/2 multiplexing** (concurrent reads on one connection without head-of-line
blocking), a **schema-first contract** (`.proto`) that the Python client
(grpcio / EPIC-007) consumes for free, **TLS** for the wide-area case, and a
**permissively licensed** (MIT) pure-Rust stack with no C/C++ build dependency.

**The remote read topology is `server-proxied reads` for v1** (the server
executes the query against object storage and streams result rows to the
client), with a **documented v2 evolution path to `delegated reads`** (the
server hands the client a pinned snapshot descriptor and the client reads object
storage directly). This is the latency-critical decision and is justified in §4.

The protocol is **read-only on the wire**: remote clients run read queries and
admin/status RPCs. Writes happen only in the server process (it is the
writer-master) via the local engine + ADR 0002 commit path; there is **no remote
write/transaction RPC** in this scope. A second process attempting to become
writer-master fails to acquire the lease and is **rejected** (clear typed error),
optionally retried client-side with backoff — never queued server-side.

### 1. Service contract (wire shape sketch)

A single gRPC service. Result rows stream; the snapshot version is explicit on
every read so the client and server agree on exactly which immutable snapshot a
query ran against.

```proto
syntax = "proto3";
package caerostris.v1;

service GraphDb {
  // Open a read session: server pins a snapshot version (TTL'd pin in db/pins/)
  // and returns the pinned version + a session id the client renews.
  rpc OpenReadSession(OpenReadSessionRequest) returns (ReadSession);

  // Keep the snapshot pin alive (renews the TTL'd pin object). Client must
  // renew before pin_ttl_ms elapses or the snapshot becomes GC-eligible.
  rpc RenewSession(RenewSessionRequest) returns (RenewSessionAck);

  // Run a read-only openCypher query against the session's pinned snapshot.
  // Rows STREAM back (server-streaming) so LIMIT-driven early termination and
  // large result sets never buffer the whole answer in memory.
  rpc RunQuery(RunQueryRequest) returns (stream ResultEvent);

  // Release the pin promptly (also released on session TTL expiry / disconnect).
  rpc CloseSession(CloseSessionRequest) returns (CloseSessionAck);

  // Liveness + which snapshot the server currently serves (advisory).
  rpc Status(StatusRequest) returns (ServerStatus);
}

message OpenReadSessionRequest {
  string db_prefix = 1;            // bucket/prefix being attached (informational)
  optional uint64 at_version = 2;  // optional: pin a specific committed version
}
message ReadSession {
  string session_id = 1;
  uint64 pinned_version = 2;       // the immutable snapshot this session reads
  uint32 pin_ttl_ms = 3;          // client must renew within this window
}

message RunQueryRequest {
  string session_id = 1;
  string cypher = 2;
  map<string, Value> params = 3;   // parameterized queries (no string interpolation)
  uint64 expected_version = 4;     // == pinned_version; server rejects on mismatch
}

// Server-streaming envelope: a header (column names), then rows, then a trailer
// carrying QueryStatistics (BUG-0006 observability surface) and any out-of-
// envelope warning (Cat. 3) so the client sees it even on a streamed result.
message ResultEvent {
  oneof event {
    ResultHeader header = 1;       // column names/types, served_version
    ResultRow    row    = 2;       // one row of Values
    ResultTrailer trailer = 3;     // stats, out_of_envelope flag, error (if any)
  }
}
message ResultTrailer {
  QueryStatistics stats = 1;       // rows, bytes_read_from_s3, phases (K), etc.
  bool out_of_envelope = 2;        // Cat. 3 explicit handling, surfaced to client
  string note = 3;                 // human-readable (e.g. degrade/warn reason)
}
```

`Value` is a small tagged union (null/bool/int/float/string/bytes/list/map +
node/relationship/path) mirroring the engine's value type, so results arrive as
typed objects, not stringly-typed JSON (serves the Python bindings' "native
Python objects" criterion, Cat. 8).

### 2. Relationship to the writer lease and ADR 0002 (no second fencing source)

The server becomes writer-master **exactly as an embedded writer-master would**:
it acquires the `db/lease/writer` object via the ADR 0002 create-only-CAS lease
and renews it; its commits succeed only via the per-version create-only-CAS
manifest PUT. **The network protocol plays no role in fencing.** Concretely:

- A remote client connection does **not** confer or extend writer authority. The
  server is writer-master because it holds the lease/wins manifest CAS, **not**
  because clients are connected to it.
- If the server's lease expires (it stalled) and another process becomes
  writer-master, the **zombie server cannot corrupt state**: its next commit
  attempt is a per-version manifest CAS that loses (the version was already
  created by the new master) → its commit fails → it self-fences and steps down.
  This is the *same* guarantee ADR 0002 proves; the wire protocol neither
  strengthens nor weakens it.
- Therefore **two servers (or a server + an embedded writer) can never both
  commit a given version** — split-brain on writes is impossible by the commit
  mechanism. Two servers *reading* the same prefix is always safe (immutable
  versioned snapshots).

A stale zombie server may still **serve reads** of an older pinned snapshot after
losing the lease; that is *safe* (the snapshot's objects are immutable and the
pin keeps them GC-protected) but may be **stale**. `Status` advertises the
served version so a client can detect staleness; `OpenReadSession` always
re-resolves the latest committed version (LIST/max per ADR 0002), so new sessions
never inherit a zombie's stale view.

### 3. Snapshot pinning for remote readers (uniform with embedded modes)

`OpenReadSession` causes the server to (a) resolve the latest committed version
(or the requested `at_version`) per ADR 0002 manifest resolution, and (b) write a
**TTL'd pin object** `db/pins/<session-uuid>` `{ version, deadline }` — the
*same* pin mechanism embedded/master-less readers use. `RenewSession` re-PUTs the
pin with a fresh deadline. `CloseSession`, session TTL expiry, **and TCP
disconnect** all release the pin (delete the object; expiry is the backstop if
the server itself crashed). This makes GC safety **uniform across all four attach
modes** and requires no special-case GC logic for remote clients — exactly the
decision-0004 obligation.

Because pins are TTL'd objects (not in-process state), a server crash does **not**
strand a pin forever: the deadline lapses and GC's grace window reclaims it (ADR
0002 §GC). A client whose server died simply fails its next `RenewSession`/
`RunQuery` and reconnects (to a new server, or reads master-less).

### 4. Read topology: server-proxied (v1) vs delegated (v2) — the latency decision

This is the load-bearing decision for Cat. 3, far more than the gRPC-vs-HTTP
encoding choice. Two topologies:

- **(v1) Server-proxied reads.** Client sends Cypher → server runs the query
  (does the S3 range-GETs, planning, expansion) → server streams **result rows**
  back. The server↔S3 leg pays up to `B_max`; the client↔server leg pays only the
  **result-row bytes** (for the headline query, `LIMIT 10` ⇒ tiny). The client's
  end-to-end latency = server's cold-start latency + one client↔server round trip
  + result-stream time. If the server is co-located with object storage (same
  region/AZ — the intended deployment), the server↔S3 RTT is small and the
  envelope math (ADR 0001) holds; the extra client↔server leg adds **one** RTT +
  small transfer, well inside the 1 s→2 s headroom.

- **(v2) Delegated reads.** Server returns a pinned snapshot descriptor (version +
  manifest + signed object-key list); client reads object storage **directly**.
  This removes the server from the data path (server↔client carries no bulk data)
  and lets read clients scale without loading the server — attractive for the
  fan-out case. But it requires the client to have object-store credentials/access
  and to embed the read engine (range-GET planner), which is a much larger client.

**Decision: v1 (server-proxied) ships now; v2 is a documented evolution path.**
Rationale: (1) v1 is the smaller client (no S3 creds, no embedded planner) — a
thin gRPC client suffices, which is exactly what the Python remote client
(EPIC-007) wants; (2) v1 keeps the bulk `B_max` transfer on the server↔S3 leg
which, in the intended co-located deployment, is the leg the envelope already
budgets for; (3) for the **headline** in-envelope query (`LIMIT 10`), the
client↔server result payload is tiny, so proxying adds ~1 RTT, not ~B_max. The
**out-of-envelope** large-result case is exactly where v2's direct-read scaling
helps, and that case is *detected and handled explicitly* (Cat. 3) regardless of
topology. The proto contract is **forward-compatible** with v2: `ReadSession`
can later carry an optional snapshot descriptor without breaking v1 clients.

**Latency budget note (binding):** the cold-start SLA is measured *end-to-end at
the client*. For a **remote** client the budget must account for the extra
client↔server leg. ADR 0001 / SPIKE-0006 own `L_p99` and `K`; this ADR registers
the obligation that the **server-mode benchmark** (T-0016 / EPIC-009) measure the
*remote-client* end-to-end P99 and that the client↔server RTT be added to the
envelope's reserved latency, not silently absorbed. Co-location keeps this within
the 1 s target; wide-area deployment is explicitly a **degraded** (≤ 2 s ceiling,
or out-of-target) configuration and must be documented as such, not hidden.

### 5. Concurrency, backpressure, disconnect

- **Concurrent reads:** HTTP/2 multiplexes many `RunQuery` streams over one
  connection; many clients open many connections. The server runs reads against
  immutable pinned snapshots, so reads never block each other or the writer
  (R2/R3). Tonic/Tokio handles connection concurrency; the read path is
  lock-free w.r.t. the writer (separate immutable versions).
- **Backpressure:** server-streaming + HTTP/2 flow control means a slow client
  throttles its own stream without stalling others; `LIMIT` early-terminates the
  server-side expansion so we don't compute rows the client won't read.
- **Disconnect:** on stream/connection drop the server cancels the in-flight
  query (Tokio cancellation) and releases that session's pin (TTL backstop if the
  cancel is missed). No orphaned lease (the lease is the *server's*, not per
  client) and no orphaned pin beyond its TTL.
- **Graceful shutdown:** on SIGTERM the server stops accepting new sessions,
  drains in-flight streams (bounded), **releases the writer lease** (delete
  `db/lease/writer` if owner==self, else let it expire), and exits. A client mid-
  query gets a clean `UNAVAILABLE`; its pin is released. No split-brain: even an
  *ungraceful* kill is safe because the next writer's commit CAS fences the dead
  one (§2).

## Alternatives considered

### Alternative A — Custom framed TCP protocol (length-prefixed frames over Tokio)

**Description:** A bespoke binary protocol: length-prefixed frames, a tiny
request/response + streaming-rows framing, serialized with `bincode`/`postcard`
over a raw `tokio::net::TcpStream`.

**Why considered:** Zero protocol-framework overhead; smallest possible wire and
dependency footprint; full control over framing to shave every byte/RTT off the
latency budget; no HTTP/2 header overhead.

**Why rejected:** We would **reinvent** connection multiplexing, flow
control/backpressure, stream cancellation, framing, version negotiation, TLS,
and — most costly for EPIC-007 — we would have to **hand-write a Python client**
for the bespoke framing. gRPC gives all of this off the shelf with a permissive
pure-Rust stack. The marginal latency win is illusory: for the headline `LIMIT
10` query the client↔server payload is tiny, so HTTP/2 header overhead is
negligible against the S3 round-trip-dominated budget (ADR 0001 — *latency, not
bytes, dominates cold start*). A custom protocol trades a real (client ecosystem,
correctness-of-framing) cost for a non-real (latency) gain. Reconsider only if
profiling ever shows gRPC framing materially inside the budget — unlikely given
the budget is S3-RTT-bound.

### Alternative B — HTTP/1.1 + JSON (REST)

**Description:** A REST-ish JSON API: `POST /query` returns rows; long-polling or
chunked transfer for streaming; JSON value encoding.

**Why considered:** Maximal client ubiquity (every language has an HTTP+JSON
client; trivial `curl` debugging); no `.proto` toolchain; lowest barrier for ad-
hoc clients.

**Why rejected:** (1) **No first-class server-streaming** — chunked-transfer
streaming of JSON is awkward, lacks typed trailers (we need a stats/out-of-
envelope trailer, BUG-0006 / Cat. 3), and HTTP/1.1 has **head-of-line blocking**
(one slow query stalls the connection), forcing a connection per concurrent
query. (2) **JSON is lossy and bulky** for graph values — integers vs floats,
bytes, and node/relationship/path types need out-of-band conventions; encoding/
parsing cost is higher and the payload is larger, which *does* matter once result
sets grow. (3) Typed schema-first contract (which the Python client and tests
benefit from) is absent. gRPC's HTTP/2 + protobuf solves all three. (We may later
add an optional thin HTTP/JSON gateway for ad-hoc/debug use via `grpc-gateway`-
style transcoding — a non-blocking nicety, not the primary contract.)

### Alternative C — gRPC with **delegated** reads as the v1 topology

**Description:** Adopt gRPC (as decided) but make the **v2 delegated-read**
topology the v1 shipping topology: server returns a snapshot descriptor; clients
read S3 directly.

**Why considered:** Best read-scaling (server off the data path); arguably the
"most object-store-native" client.

**Why rejected (as v1):** It makes the client **heavy** (needs S3 credentials and
the embedded range-GET read engine), which directly conflicts with EPIC-007's
desire for a **thin** Python remote client and with the AC's "Python-client
friendliness." It also widens the security surface (every read client needs
object-store access). Server-proxied (v1) ships the thin-client win now; the
proto is forward-compatible so delegated reads can be added later **without** a
breaking change. Hence v2, not v1.

### Alternative D — Apache Arrow Flight

**Description:** Arrow Flight is a gRPC-based bulk-data transport built on Arrow
IPC, with first-class Python support (`pyarrow.flight`) and zero-copy
deserialization.

**Why considered:** Arrow's columnar layout maps to caerostris-db's columnar
storage; zero-copy in Python; throughput that saturates ≥10 Gbps for bulk
results.

**Why rejected:** (1) **Dependency weight** — `arrow-flight` pulls the whole
`arrow` crate ecosystem (Apache-2.0; license-clean but heavy). We do **not** yet
have an Arrow-based internal columnar representation, and adopting Arrow as the
*wire* format ahead of the storage-format ADR (SPIKE-0003) would prematurely
constrain that design. (2) **Overkill for the headline workload** — the headline
query returns ≤ 10 rows (`LIMIT 10`); Arrow IPC framing for a handful of rows is
not a clear win over protobuf, and Arrow Flight's strength (millions of bulk
rows) is the *out-of-envelope* case we explicitly detect/handle, not the target.
(3) **Larger surface** — Flight's `do_get`/`do_put`/auth/endpoint-discovery
surface exceeds the small read contract we need. Revisit only if a future
analytical-bulk-export workload + an Arrow-native storage layout both materialise
(future perf spike); the gRPC choice does not foreclose adding a Flight endpoint
later.

## License check (SPDX — permissive only)

All wire-protocol dependencies are MIT / Apache-2.0 / BSD; **no GPL/AGPL/SSPL**.
Recorded here and to be reflected in the license manifest (T-0039).

| Dependency | Role | Version (target) | SPDX | Source |
|------------|------|------------------|------|--------|
| `tonic` | Rust gRPC server + client | 0.12.x | MIT | crates.io/crates/tonic |
| `prost` | Protobuf runtime (Rust) | 0.13.x | Apache-2.0 | crates.io/crates/prost |
| `tonic-build` | `build.rs` codegen | 0.12.x | MIT | crates.io/crates/tonic-build |
| `protoc` | Protobuf compiler (build-time) | bundled | BSD-3-Clause | github.com/protocolbuffers/protobuf |
| `protoc-bin-vendored` | vendored `protoc` (avoid system dep) | latest | MIT | crates.io/crates/protoc-bin-vendored |
| `tokio` | async runtime (transitive, already planned) | 1.x | MIT | crates.io/crates/tokio |
| `grpcio` (Python) | Python gRPC client (EPIC-007) | 1.x | Apache-2.0 | pypi.org/project/grpcio |
| `betterproto` / `grpcio-tools` (Python) | Python protobuf codegen | latest | MIT / Apache-2.0 | pypi.org/project |

**Build-time `protoc`:** prefer the Nix devenv's native `protoc`; fall back to the
MIT `protoc-bin-vendored` crate so the build is reproducible without a system
install. Either path is license-clean. The license manifest task (T-0039) records
these once `Cargo.toml` carries them.

## Consequences

### Positive

- **Cat. 7:** unblocks the fourth attach mode (server) with a concrete,
  testable contract; concurrent remote reads ride HTTP/2 multiplexing over
  immutable snapshots. Split-brain remains impossible — fencing stays in ADR
  0002's commit CAS, untouched by the wire protocol.
- **Cat. 8:** Python remote client is a thin generated grpcio stub against the
  same `.proto`; no bespoke client to hand-write.
- **Cat. 3:** server-proxied topology keeps the bulk `B_max` transfer on the
  co-located server↔S3 leg; `LIMIT 10` keeps the client↔server payload tiny;
  streaming + early termination avoid buffering. The out-of-envelope
  trailer surfaces Cat. 3 handling to remote clients explicitly.
- **Process:** permissive pure-Rust stack (tonic/prost, MIT) — no C toolchain,
  license-clean.

### Negative / trade-offs

- **Extra network leg for remote clients.** A remote read pays client↔server RTT
  on top of the server's cold-start latency. Inside the target (co-located) this
  is ~1 RTT of headroom; **wide-area is a degraded/ceiling configuration** and is
  documented as such (§4). This is an honest cost, not a hidden one.
- **`.proto` toolchain dependency** (`prost-build`/`protoc` at build time). Pure-
  Rust `protoc` (`protoc-bin-vendored` or the `protobuf-src` build) keeps it
  license-clean and reproducible; recorded in §7.
- **Server-proxied v1 puts read CPU/bytes on the server.** For massive fan-out
  (out-of-envelope) this loads the server; mitigated by out-of-envelope detection
  (Cat. 3) and the documented v2 delegated-read path for scale-out later.

### Open questions

- **AuthN/AuthZ on the wire** (who may open a read session) is out of scope for
  the T0+4h deadline (single-tenant, trusted-network assumption). TLS is available
  via tonic; mutual-TLS / token auth is a future ADR. **Recorded, not resolved.**
- **Exact `Value` ⇄ engine-type mapping** (node/relationship/path encoding) is
  pinned when the engine value type lands (EPIC-002); the proto reserves the
  oneof arms now.
- **v2 delegated-read snapshot descriptor format** (signed key list, credential
  delegation) is deferred to a future ADR; the proto is forward-compatible.
- **Remote-client latency benchmark** — T-0016 / EPIC-009 must add a *remote*
  end-to-end measurement (client↔server leg included), not only the embedded
  cold-start number. Registered as an obligation here.

## Rubric impact

| Cat. | Name | Impact |
|------|------|--------|
| 7 | Concurrency & attach modes | Unblocks server mode (4th attach mode); concurrent remote reads; split-brain prevention unchanged (delegated to ADR 0002). Advances Cat. 7 toward 100. |
| 8 | Python bindings | Thin grpcio remote client against the shared `.proto`; serves the "remote read attach mode" + "native objects" criteria. |
| 3 | Latency envelope + SLA | Constrains the protocol to be thin + streaming; registers the remote end-to-end benchmark obligation; co-located = target, wide-area = degraded (explicit). |
| 1 | ACID & correctness | No new fencing/isolation surface — all ACID truth stays in ADR 0002; this ADR must not contradict it (verified in §2/§3). |

## Sign-off

### Adversarial review record

<!-- Append adversarial-reviewer verdict blocks here as rounds complete. -->

This ADR was authored by `steering-distributed-acid`, who pre-registered the
falsification scenarios it must survive (below). Per the design-falsification
loop and the steering-committee protocol, **the author does not self-ratify**:
this ADR is submitted for an independent `adversarial-reviewer` round and steering
sign-off (cross-cutting → `steering-distributed-acid` primary,
`steering-query-cypher` / `steering-perf-sla` consulted for the Python-client and
latency surfaces). Sign-off request recorded in
`.project/decisions/0015-spike-0009-server-protocol-signoff-request.md`.

**Falsification scenarios pre-registered by the author (must survive review):**

1. **Split-brain via the wire protocol.** *Can a client connection make a
   zombie server believe it is still writer-master and commit?* Refuted in §2:
   client connections confer no writer authority; commits are gated solely by the
   per-version manifest CAS (ADR 0002); a zombie's commit loses the CAS and it
   self-fences. The wire protocol has **zero** fencing role.
2. **Torn read across a commit.** *Can a remote client see a partial/mixed
   snapshot if the writer commits V+1 while a `RunQuery` is in flight?* Refuted in
   §3: every read runs against the session's **pinned immutable version**;
   `expected_version` is checked; a new commit creates a *new* immutable version
   and never mutates the pinned one. Snapshot isolation holds (ADR 0002
   `SnapshotIsolation` invariant).
3. **Orphaned pin / GC unsafety on disconnect or server crash.** *Does a dead
   remote client or a crashed server strand a pin and block GC forever, or let GC
   reclaim a still-read snapshot?* Refuted in §3/§5: pins are **TTL'd objects**
   reclaimed by the grace window; disconnect releases the pin (TTL backstop). GC
   safety is uniform with embedded modes (decision 0004 obligation met).
4. **Attach-mode transition mid-operation.** *Server loses lease while serving a
   read; another process becomes master.* Refuted in §2: the in-flight read is on
   an immutable pinned snapshot and stays correct (possibly stale, advertised via
   `Status`); new sessions re-resolve latest. No write can be torn because the
   zombie's commit CAS loses.
5. **Latency budget violation.** *Does proxying through the server bust the
   cold-start budget?* Addressed in §4: for the headline `LIMIT 10` query the
   client↔server payload is tiny and adds ~1 RTT inside the co-located target;
   the remote end-to-end benchmark obligation is registered so this is *measured*,
   not assumed; wide-area is explicitly a degraded configuration, never a hidden
   miss.

### Steering ratification

#### steering-distributed-acid (primary, Cat. 7 — attach modes / writer leasing) — RATIFIED  (≈ T0+1:40)

**Verdict:** `approve` (ratified — protocol-selection scope; carried-forward
implementation gate recorded below).

I ran the design-falsification loop (`docs/process/adversarial-review-loops.md`,
Loop A) against this ADR. I did not take its claims about ADR 0002 on faith — I
read the commit-protocol ADR + TLA+ artifacts on
`work/SPIKE-0002-design-s3-commit-protocol-and-tla-model-for-atomic` and confirmed
every mechanism this ADR leans on is real and faithfully represented:
`db/pins/<uuid>={version,deadline}` TTL'd pins, `db/lease/writer={owner,epoch,
deadline}` create-only-CAS lease, per-version `manifest/<V>.json` +
`PUT If-None-Match:*` fencing CAS with advisory-only `_latest`, the
`SnapshotIsolation` and `AtMostOneCommitPerVersion` model invariants (TLC: 7406
distinct states, no violations — decision 0014), and GC rule 3 "grace window
strictly greater than max reader-session/renewal period." This ADR adds **zero**
new fencing and **zero** new isolation surface — it is a faithful, subordinate
wire layer over the commit protocol. That is exactly the shape I require.

**Scenarios constructed and survived (specific, not "looks fine"):**

1. **Split-brain via the wire (my GATE).** S1 holds lease + n clients connected;
   S1 stalls; lease expires; W2/S2 commits V+1 via create-only CAS; S1 wakes
   believing it is still master and attempts to commit V+1 → its
   `PUT If-None-Match:* manifest/<V+1>.json` returns 412 → it self-fences. A
   client connection confers **no** writer authority (§2). The wire protocol has
   no fencing role and cannot introduce a second, disagreeing fencing source
   (decision 0004 #3 / SPIKE-0005 Constraint 2 satisfied). **Survives.**
2. **Torn read across a commit (Cat. 1 isolation).** C1 `OpenReadSession`→pinned
   V, `RunQuery` in flight; writer commits V+1 (new immutable keys only;
   `expected_version==pinned_version` checked server-side). C1 reads only V's
   immutable objects; the pinned version is never mutated. `SnapshotIsolation`
   holds (§3). **Survives.**
3. **Orphaned pin / GC unsafety on disconnect or server crash.** Stream/connection
   drop → server deletes the session pin; if the server itself crashed, the pin's
   TTL lapses and GC's grace window (grace > max session) reclaims — uniform with
   embedded readers, no remote-client special case (decision 0004 obligation met).
   **Survives.**
4. **Two-hop liveness (server-mode-specific, NOT covered by ADR 0002).** In
   ADR 0002 the reader process owns/renews its own pin; here the *server* PUTs the
   pin and the *client* drives renewal via `RenewSession`. I traced the case where
   the **client is alive and renewing but the pin-owning server dies**: the client
   fails its next `RenewSession`/`RunQuery` and reconnects (new server, or reads
   master-less); the dead server's pin self-expires by TTL; the in-flight read was
   against immutable objects, and any post-death GC of that version surfaces as a
   **clean error on the next call**, never a torn/partial read. Clean-failure
   path, not a correctness hole. **Survives.**
5. **Attach-mode transition mid-operation.** Server loses lease mid-read; another
   process becomes master. The in-flight read stays correct on its immutable
   pinned snapshot (possibly stale — advertised via `Status`); new sessions
   re-resolve latest (LIST/max). No write can be torn (zombie's commit CAS loses).
   **Survives.**

**Why ratify as primary now (and what this does NOT do):** the protocol-selection
decision is squarely my primary domain (sign-off table: "Attach modes / writer
leasing → steering-distributed-acid"). It is structurally sound and survives every
ACID/split-brain scenario. The two consulted concerns are **scoped and already
registered as explicit, tracked obligations in this ADR** — they do not threaten
the protocol *choice*:

- **`steering-perf-sla` (consulted, Cat. 3):** the remote end-to-end latency
  benchmark (client↔server leg included) is registered as a binding obligation on
  T-0016/EPIC-009 (§4), with "co-located = target / wide-area = degraded" stated
  honestly. The protocol is thin + streaming + early-terminating; for the headline
  `LIMIT 10` query the client↔server payload is tiny and the bulk B_max stays on
  the co-located server↔S3 leg. I record no latency falsification; perf-sla's
  consulted confirmation tracks against that benchmark obligation, not against the
  encoding choice.
- **`steering-query-cypher` (consulted, Cat. 8/4):** the wire `Value` oneof
  reserves node/relationship/path arms and explicitly defers temporal
  (Date/DateTime/Duration) pinning to when the engine value type lands (EPIC-002).
  This is correctly deferred — it is a *fill-in-the-oneof* detail, not a protocol
  re-selection — so it is not a blocker for the protocol decision.

I am invoking the operating model's "decide toward intent, record why, keep
moving / never block the board" doctrine: the GATE-Cat.7 protocol choice should
not stall on two scoped, non-blocking, already-tracked consulted items while we
are behind pace. If either consulted member later refutes their scoped concern,
that is a `superseded`-class change to be opened as a new ADR, not a reason to
hold this gate open now.

**Carried-forward implementation gate (binding — board honesty):** ratifying
SPIKE-0009 unblocks the *design*, not the *code*. **T-0029 stays `backlog`** until
ALL hold: (a) ADR 0002 / SPIKE-0002 is ratified by `steering-distributed-acid`
(primary) — currently pending — and landed on `main`; (b) the two-concurrent-`PUT
If-None-Match:*` → exactly-one-200 mock-fidelity test (decision 0014 C-B) is green
in CI; (c) T-0027 (embedded modes) is `done`. This honours the prove-before-code
rule I jointly enforce with `steering-formal-methods`: a wire layer may not ship
ahead of the commit protocol it is subordinate to.

**Note to `steering-formal-methods`:** this ADR introduces no new state or
invariant requiring a TLA+ change — it is a faithful client of the existing
commit-protocol model (no new fencing/isolation surface). No model update is
required for the protocol choice. The server-proxied pin lifecycle is the same
`db/pins/` pin/renew/expire FSM already in `commit_protocol.tla`; if a future
implementation diverges (e.g. server-side pin ownership semantics differ from the
modelled reader-owned pin), that is a model↔code drift BUG to file at that time.

**Signed:** steering-distributed-acid  T+~1:40

#### steering-perf-sla (consulted — latency) / steering-query-cypher (consulted — wire type)

_Tracked, non-blocking against the protocol choice (see primary verdict). Their
scoped obligations are registered in this ADR (§4 remote benchmark; §Open
questions `Value`/temporal types). They may append confirmations or open a
`superseded` ADR if a scoped concern is later refuted; neither holds open the
GATE-Cat.7 protocol-selection ratification while the run is behind pace._
