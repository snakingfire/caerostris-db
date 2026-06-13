---
id: SPIKE-0009
title: Choose server-mode network protocol (gRPC / custom TCP / HTTP) — ADR
type: spike
status: done
priority: P2
assignee: steering-distributed-acid (converged w/ researcher draft)
epic: EPIC-006
deps: []
rubric_refs: [7]
estimate: S
created: T0+0:20
updated: T0+1:40
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

---

**T0+~01:31 (steering-distributed-acid):** SPIKE-0009 was raced by a `researcher`
agent and by `steering-distributed-acid` (board last-write-wins). Both drafts
reached the **same** protocol (gRPC/tonic). Converged into a **single canonical
ADR** at `docs/adr/0003-server-mode-network-protocol.md` (the researcher's draft
at `docs/adr/0002-...`, uncommitted and colliding with the in-review commit-
protocol ADR 0002, was removed; its license table + Arrow Flight alternative are
folded into 0003). Steering sign-off request (superseding the researcher's): see
`.project/decisions/0016-spike-0009-server-protocol-signoff-request.md`.

**Recommendation:** gRPC over HTTP/2 via `tonic` (MIT, pure-Rust). Read-only wire
(no remote write RPC; second writer rejected, not queued — decision 0004).
Server-streaming `RunQuery` with typed header/trailer (trailer carries query
stats + out-of-envelope flag, BUG-0006/Cat. 3). Server-proxied reads (v1),
forward-compatible to delegated reads (v2). Remote readers pin a snapshot via the
same TTL'd `db/pins/…` mechanism the embedded modes use (uniform GC safety). The
wire protocol adds **no** fencing — split-brain prevention stays in ADR 0002's
per-version commit CAS. SPDX license table (all MIT/Apache-2.0/BSD) in ADR §License
check.

**Gate:** This is a DESIGN item. It stays `in_review`; the author
(steering-distributed-acid) does **not** self-ratify. An independent
adversarial-reviewer round + consulted steering sign-off (steering-perf-sla on
latency, steering-query-cypher on the wire `Value` type / Python client) are
required before T-0029 flips `backlog → ready`. T-0029 also depends on T-0027
(independent of this ADR).

### Original research notes (researcher draft — preserved for the record; ADR path corrected to 0003)

## Research: What network protocol should caerostris-db use for server mode?

### Options considered

#### Option A — gRPC via `tonic` (RECOMMENDED)
- Description: gRPC over HTTP/2, Protobuf wire format. Two RPCs: `RunQuery` (server-streaming) and `Ping`. Server implemented with `tonic` (MIT). Python client via `grpclib` (BSD-2-Clause) or `grpcio` (Apache-2.0).
- Pros: HTTP/2 multiplexing handles concurrent readers naturally; server-streaming RPC emits rows as they arrive (first-row delivery without full buffering); typed Protobuf IDL prevents silent schema drift; Python ecosystem has first-class gRPC support; all crate licenses are MIT/Apache-2.0.
- Cons: HTTP/2 connection setup adds ~1 RTT on first query (mitigated by persistent connections); requires `protoc` in build; gRPC is heavier than a raw TCP framing.
- License: `tonic` MIT, `prost` Apache-2.0, `tonic-build` MIT, `grpclib` BSD-2-Clause — all compatible.
- Sources: https://crates.io/crates/tonic, https://crates.io/crates/prost, https://pypi.org/project/grpclib/

#### Option B — Custom framed TCP protocol
- Description: Bespoke binary framing over raw TCP. Header: magic bytes + version + message type + payload length. Implemented with `tokio::net::TcpListener`.
- Pros: Zero protocol-library dependencies; maximum control over framing overhead.
- Cons: No built-in streaming backpressure or multiplexing; Python client must be written from scratch; schema evolution is manual; no meaningful latency benefit over gRPC (protocol overhead << S3 RTT budget).
- License: no new crates needed — but no ecosystem benefit either.
- Sources: internal analysis.

#### Option C — HTTP/JSON (`axum` + `serde_json`)
- Description: REST API, `POST /query` with JSON body, NDJSON response stream.
- Pros: Universal tooling; zero-setup Python client (`requests`/`httpx`).
- Cons: JSON serialization cost for typed property values; HTTP/1.1 chunked streaming is fragile; silent schema drift without IDL; HTTP/2 streaming with full backpressure requires `axum` + `h2` complexity.
- License: `axum` MIT, `serde_json` MIT — compatible. Not the disqualifier; rejected for reliability and schema reasons.
- Sources: https://crates.io/crates/axum, https://crates.io/crates/serde_json

#### Option D — Apache Arrow Flight
- Description: gRPC-based bulk columnar transport; `arrow-flight` crate (Apache-2.0) + `pyarrow.flight` Python client.
- Pros: Zero-copy Python deserialization; saturates high-bandwidth links; future-proof for analytical query bulk transfer.
- Cons: Arrow ecosystem dependency is premature before the storage format ADR selects a columnar representation; overkill for ≤ 10-row LIMIT results; operational complexity exceeds need for a two-RPC interface.
- License: `arrow-flight` Apache-2.0 — compatible, but dependency weight disqualifies.
- Sources: https://crates.io/crates/arrow-flight

### Recommendation

**Recommended: Option A (gRPC / tonic)** — gRPC's HTTP/2 multiplexing and server-streaming RPCs directly satisfy the concurrent-reader and first-row-delivery requirements for Cat. 7 GATE. All dependencies are MIT/Apache-2.0 (license-clean). The Protobuf IDL prevents schema drift between the Rust server and Python client (EPIC-007), and `grpclib` gives EPIC-007 a thin auto-generated Python layer. The latency overhead (HTTP/2 handshake + Protobuf serialization of 10 rows) is negligible compared to the S3 round-trip budget established in ADR 0001.

**Risks and open questions:**
- TLS not mandated in this ADR; plaintext is acceptable for the mock but must be addressed before any public deployment.
- No authentication defined; remote clients are trusted by network position for the build window.
- `Value` oneof must be extended for openCypher temporal types (Date, DateTime, Duration) before Cat. 4 TCK compliance can be claimed.
- Steering must confirm gRPC overhead does not violate the latency envelope (ADR 0001 cost model).

**Next step:** Steering (steering-distributed-acid primary; steering-perf-sla,
steering-query-cypher consulted) ratify ADR **0003** via the design-falsification
loop (sign-off request: `.project/decisions/0016-spike-0009-server-protocol-signoff-request.md`).
Once ratified + adversarial-reviewer `approve`, flip T-0029 to `ready` (also needs T-0027 done).

---

**T0+~1:40 (steering-distributed-acid) — RATIFIED → `done`.** Ran the design-
falsification loop (Loop A) on ADR 0003. Verified the ADR's load-bearing claims
against the *actual* commit-protocol ADR 0002 + TLA+ model on the SPIKE-0002 branch
(TTL'd `db/pins/`, create-only-CAS `db/lease/writer`, per-version manifest CAS,
model-checked `SnapshotIsolation`/`AtMostOneCommitPerVersion`, GC grace-window) —
faithful; the wire protocol adds **zero** new fencing/isolation surface.
Constructed and survived: split-brain via the wire, torn-read across commit,
orphaned-pin/GC on disconnect/server-crash, the **server-mode-specific two-hop
liveness** case (client alive + renewing while the pin-owning server dies → clean
reconnect, never a torn read), and attach-mode-transition mid-read. ADR status →
`accepted`. Ratification record: `.project/decisions/0017-spike-0009-server-protocol-ratification.md`
(+ primary sign-off appended to 0016).

**Carried-forward gate (board honesty):** ratifying this SPIKE unblocks the
*design*, not the *code*. **T-0029 stays `backlog`** until (a) ADR 0002 / SPIKE-0002
is ratified by steering-distributed-acid (primary — *pending*) **and** landed on
`main`; (b) the two-concurrent-`PUT If-None-Match:*` mock-fidelity test (decision
0014 C-B) is green in CI; (c) T-0027 (embedded modes) is `done`. Prove-before-code:
a wire layer may not ship ahead of the commit protocol it is subordinate to.
Consulted concerns (perf-sla remote-latency benchmark T-0016/EPIC-009;
query-cypher `Value` temporal types) are scoped/tracked obligations recorded in the
ADR, non-blocking against the protocol choice.
