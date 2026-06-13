# SPIKE-0008 — Storage-domain falsification constraints

**Status:** done — constraints fully specified; discharge obligations forwarded to
SPIKE-0002 and SPIKE-0003.

**Date / T+ marker:** 2026-06-13T19:10:00Z (T+0:46)

**Author:** researcher

**Feeds:** SPIKE-0003 (storage format spec), SPIKE-0002 (commit protocol ADR + TLA+)
and their dependent implementation tasks T-0007, T-0008, T-0009, T-0010, T-0011, T-0012.

---

## Research question restated

What are the **precise, technically complete discharge obligations** that SPIKE-0002
and SPIKE-0003 must satisfy for findings F1, F2, and F3 (surfaced by
`steering-storage` during the day-one ratification pass; tracked in
`.project/decisions/0001-storage-domain-ratification-findings.md`) — specific enough
that the Cat. 2 [GATE] can be honestly scored 100?

---

## Background

`steering-storage` approved the commander's intent and master rubric (the design is
structurally feasible) but flagged three under-specifications that must be closed
before the storage-format ADR (SPIKE-0003 output) can be ratified and before
Cat. 1/2 [GATE] categories can be honestly scored 100. The findings are listed in
SPIKE-0008 acceptance criteria; this document makes each precise enough to act on.

---

## F1 — Early-abort partial adjacency reads (binding 50 Mbps case)

### Why this is a gate-level constraint

At 50 Mbps the byte budget B_max ≈ 4 MB (derivation: 50 Mbps = 6.25 MB/s;
reserving K·L_p99 ≈ 0.4 s for K=3 round trips at L_p99 ≈ 130 ms plus ~0.05 s
compute leaves ~0.55 s transfer time; 6.25 × 0.55 ≈ 3.44 MB, call it 4 MB
including overheads). The naive product-bound for a 6-hop BFS with worst-case
fan-out f=10:

    |seed| * f^6 = |seed| * 1,000,000

Even with a highly selective filter yielding |seed| = 1 and an average node-edge
record size of just 16 bytes, the realized 6-hop frontier would read up to
16 MB of adjacency data — 4× the budget. At f=5 (more realistic with LIMIT-driven
pruning) and a realized frontier of ≈ 15,625 terminal nodes, the frontier adjacency
data alone is still ≥ 250 KB per hop, totalling well over 4 MB for 6 hops.

The proof in SPIKE-0001 must therefore **not** assume full BFS expansion; it must
depend on LIMIT-driven early termination that prunes the realized frontier. And the
on-object layout must support **aborting an adjacency range-GET mid-stream** once
the LIMIT is satisfied — otherwise the network read completes even though the engine
discards most bytes.

### Precise discharge obligations for SPIKE-0003

**F1-S1 — Adjacency-list chunking with stated maximum chunk size:**

The format spec MUST define a `max_chunk_bytes` parameter (recommended: 64 KB –
512 KB; the exact value is SPIKE-0003's design choice but must be derived from the
budget analysis). A single adjacency-list chunk covers a contiguous run of source
node IDs and contains all out-edges for those nodes. The size of one chunk must
be ≤ `max_chunk_bytes`, so a reader reading one chunk commits at most
`max_chunk_bytes` to the network before it can decide whether to abort.

The spec must state: "a reader that has reached its LIMIT after reading the first
`k` complete adjacency entries in a chunk may close the HTTP range-GET connection
without fetching the remainder of the chunk; the bytes saved are
`chunk_size − k * avg_edge_record_bytes`."

**F1-S2 — Object-range alignment that enables partial GETs:**

Adjacency-list objects must be divided into independently seekable chunks (by byte
offset within the object), where each chunk boundary is recorded in the object
header or in a chunk-index segment prepended to the object. A reader MUST be able
to:
1. Read the chunk-index (a small fixed-size header or prefix) to determine which
   byte range covers the edges for a given set of source node IDs.
2. Issue an HTTP Range GET for exactly those bytes.
3. Stop the GET early (close the connection) once LIMIT is satisfied.

The spec must define the chunk-index format (at minimum: a sorted array of
`(source_node_id_start, byte_offset)` pairs so a reader can binary-search to find
the range for a given source ID).

**F1-S3 — Worst-case bytes-per-hop bound stated explicitly:**

The spec must include a table or formula that gives, for the chosen `max_chunk_bytes`
and a representative edge record size, the **maximum bytes committed to the
network per hop** before early abort can fire. This bound must be ≤ B_max / K so
that even a single-hop read does not exhaust the budget. Example formula:

    bytes_per_hop_worst_case = max_chunk_bytes * |frontier| / nodes_per_chunk

where `nodes_per_chunk = max_chunk_bytes / avg_edge_record_bytes`. The spec must
show this stays ≤ B_max / K for the in-envelope seed set size derived by SPIKE-0001.

**F1-S4 — LIMIT-driven early termination as a first-class design element:**

The spec must explicitly state that early termination is **not** an optimization
hint but a **correctness obligation for the 50 Mbps envelope**. The access pattern
section must describe: "when the hop-expansion executor has collected LIMIT results,
it issues no further adjacency range GETs for the current frontier layer." This
language must appear in the spec so the executor implementer knows this is required,
not optional.

### Precise discharge obligations for SPIKE-0001

**F1-E1 — Realized-fan-out assumption stated, not worst-case product:**

The cost model must explicitly distinguish `f_worst = max degree` from `f_realized
= f_worst * (LIMIT / total_matching_paths)` (or a tighter analytical bound). The
P99 proof must be carried out with `f_realized`, not `f_worst^6`. The cost model
section must include a sentence of the form: "we assume `f_realized ≤ R` for some
stated R ≪ f_worst^6; this follows from LIMIT-driven pruning under the condition
that the query's LIMIT is ≤ L." Both R and L must be numeric.

**F1-E2 — Cross-reference to SPIKE-0003 adjacency chunking:**

The cost model must cite SPIKE-0003's `max_chunk_bytes` value and note: "each hop
reads at most `max_chunk_bytes * frontier_slabs` bytes before abort; the proof
holds because this ≤ B_max / K." The proof is thus co-dependent on the SPIKE-0003
spec value — if that value changes, the proof must be re-checked.

---

## F2 — Atomic manifest swap depends on a named conditional-PUT primitive

### Why this is a gate-level constraint

The Cat. 1 and Cat. 2 GATE anchor "commit = atomic manifest swap" is only as
strong as the underlying object-store primitive. There are several distinct
mechanisms, with different semantics, availability, and mock support:

| Primitive | Semantics | S3 support | MinIO support | moto support |
|---|---|---|---|---|
| `PUT If-None-Match: *` | Create-if-absent (key must not exist) | Yes (2024 conditional writes) | Yes | Partial (version-dependent) |
| `PUT If-Match: <etag>` | Overwrite only if current ETag matches (true CAS) | Yes (2024 conditional writes) | Yes (recent) | Partial |
| Uniquely-named immutable manifest + CAS pointer object | Append-only manifest objects named by content/version; a "latest" pointer swapped via `If-Match` | Yes | Yes | As above |
| Uniquely-named immutable manifests only (list/max resolution) | Each manifest has a monotonically increasing name; "latest" is resolved by listing and taking max | Yes (eventual list consistency *was* an issue; S3 now has strong read-after-write) | Yes | Yes (list is consistent in moto) |

The design MUST choose exactly one mechanism and pin it. "If-none-match or
equivalent" is not a choice — it is ambiguity that cannot be formally modelled.

### Options analysis

**Option A — Uniquely-named immutable manifests + list/max resolution**

- How: each committed manifest is written to a unique, monotonically-named object
  (e.g. `manifest/v{seq:020}.bin`). The "latest" version is the one with the
  lexicographically greatest name. A writer generating v{N+1} only proceeds if it
  still holds the lease and no v{N+1} object exists yet (verified by a `HEAD`
  before the write, or by using `If-None-Match: *` on the unique name).
- Pros: no CAS on an existing object needed; S3 `If-None-Match: *` on a new unique
  key is universally supported; listing is consistent on modern S3/MinIO; simple
  to model in TLA+.
- Cons: "latest" resolution via list introduces a read cost (one LIST call per
  reader open); list results must be consistent (modern S3 guarantees this); slow
  readers may see a stale "latest" if they cached the list result.
- License: not a dependency — pure protocol design.

**Option B — CAS pointer object via `PUT If-Match: <etag>`**

- How: a single mutable `manifest/HEAD` object holds the current manifest
  reference. Writers swap it with `PUT If-Match: <current-etag>`. The ETag changes
  atomically; exactly one concurrent swap wins.
- Pros: true CAS semantics; the safety invariant "at most one commit per manifest
  version" maps directly to ETag linearity; clean TLA+ model.
- Cons: `PUT If-Match` (conditional overwrite of an existing object) requires S3
  conditional writes (available on AWS S3 since Nov 2024, but **not** on all
  S3-compatible stores); MinIO support requires version ≥ RELEASE.2024-11-x;
  moto support is version-dependent and must be verified by an integration test.
- License: not a dependency.

**Option C — Lease-only (heartbeat object, writer belief)**

- How: writer writes a "lease" object with its ID; other writers check before
  writing. No CAS on the manifest itself.
- Cons: zombie-writer falsification (W1 stalls, lease expires, W2 commits, W1
  wakes and swaps) — this is a split-brain and a Cat. 1 GATE failure. **This is
  the option SPIKE-0005 Constraint 2 specifically rules out.** Rejected.
- License: irrelevant (rejected).

### Recommendation for SPIKE-0002

Use **Option A (uniquely-named immutable manifests + list/max)** as the primary
mechanism, with `If-None-Match: *` on each unique name as the append-guard. This
is the most universally supported pattern across S3, MinIO, and moto, avoids
depending on `PUT If-Match` availability, and yields a clean "no two writers can
produce the same unique name simultaneously" invariant.

The fencing token (from the lease) is carried into the manifest name or content
(e.g. `manifest/epoch-{E}-v{N}.bin`) so a stale writer's manifest would either
conflict on the name (if E matches) or be ignored by readers (if E is stale). The
"at most one commit per manifest version" safety invariant holds because two writers
cannot both successfully `PUT If-None-Match: *` the same key.

### Precise discharge obligations for SPIKE-0002

**F2-P1 — Name the exact primitive in the ADR:**

The commit-protocol ADR MUST contain a table equivalent to the one above and
MUST declare: "we use [Option A / Option B] with request shape [exact HTTP
header(s)]." No option-ality or "or equivalent." The TLA+ model must model the
chosen primitive exactly.

**F2-P2 — Mock-fidelity integration test specified and planned:**

The ADR must include a section "Mock-fidelity verification" that describes the test
to be run against the CI S3 mock (MinIO + moto):

    test: two concurrent goroutines/threads each attempt PUT If-None-Match:*
          on the same key (or the same unique manifest name); exactly one
          must receive 200/OK, the other must receive 412 Precondition Failed
          (or 409 Conflict, depending on the API).

This test must be cross-referenced from T-0010 (atomic manifest swap task) so the
implementer knows to run it before claiming done. If the test fails (mock does not
enforce the conditional semantic), the finding escalates immediately to
`steering-storage` + `steering-distributed-acid` — do not proceed with the mock
until resolved.

**F2-P3 — CAS safety invariant restated per SPIKE-0005 Constraint 2:**

The TLA+ invariant must NOT be `writer_count ≤ 1` (transient concurrent belief is
acceptable). It must be: "no two distinct successful commits share the same
predecessor manifest version" (equivalently: the manifest version sequence is
a linear chain with no branches). In TLA+ notation, something like:

    INVARIANT \A v1, v2 \in committed_versions :
        v1 # v2 => ~(predecessor(v1) = predecessor(v2))

The zombie/stale-writer scenario (W1 commits V+1, then W2 also tries to commit
V+1 with the same predecessor) must appear as a modelled execution and the
invariant must show W2's commit is rejected.

**F2-P4 — Fallback or hard precondition for stores without the chosen primitive:**

If Option A is chosen: document that `If-None-Match: *` on a new unique key is
treated as a hard precondition — stores that do not support it are not supported
without modification. The crate's `ObjectStore` abstraction (T-0001) must expose
a `supports_conditional_put()` capability check, and the engine must return a
clear error on open if the check fails.

---

## F3 — GC safety against slow/crashed readers with no central pin registry

### Why this is a gate-level constraint

The Cat. 2 "100" anchor requires "manifest swap atomic & concurrent-reader-safe."
R3 modes 2 (embedded read-only) and 3 (embedded master-less) mean there is no
always-live coordinator the GC can poll for reader liveness. A GC that deletes
objects while a reader is mid-range-GET on them produces 404s or corrupted reads —
a Cat. 1 + Cat. 2 failure. The three specific scenarios that must be addressed:

**Scenario A — Crashed reader with stale pin:**  
Reader R pins version V by writing a "pin object" (`pins/reader-{uuid}/v{N}.pin`)
and then crashes before releasing the pin. GC sees the pin and must not delete
objects for V until the pin is either explicitly released or has expired beyond the
reader-session lifetime upper bound.

**Scenario B — Slow reader whose pin GC cannot observe:**  
Reader R resolves the latest manifest (version V) and begins reading objects, but
has not yet written its pin object. GC runs and deletes objects for V. R then gets
a 404.

**Scenario C — GC deleting an object a reader is mid-range-GET on:**  
S3 object deletion is eventually consistent in some configurations (though modern
S3 has strong consistency for object deletes too). The main risk: GC issues a
DELETE for an object that a reader is in the middle of a multi-part range GET on.
The GET will succeed for the bytes already in flight but may fail or return partial
data if the connection is reset.

### Options analysis

**Option A — Retention grace window (no explicit pin objects)**

- How: GC never deletes a version newer than `retention_window` (e.g. 30 minutes).
  Any reader that takes longer than 30 minutes on one query is out of the supported
  session lifetime. No pin objects required.
- Pros: simple, no extra round trips for readers; no pin-GC interaction to model.
- Cons: slow queries or long-running readers (e.g. analytics) may exceed the window
  and get 404s; the window must be set conservatively; can accumulate a lot of old
  version data on a busy writer.
- Addresses Scenario A: yes (if reader session ≤ window). Scenario B: yes (any
  version resolved within the window is safe). Scenario C: yes (objects are never
  deleted while young).

**Option B — TTL'd pin objects with deletion deadline strictly after max session lifetime**

- How: reader writes a "pin" object to the object store immediately after resolving
  the manifest, before reading any data objects. The pin object has a TTL and an
  expiry timestamp that is `now + max_session_lifetime`. GC checks for existing pins
  before deleting any version; if a pin exists and has not expired, GC skips that
  version. A crashed reader's pin expires automatically after `max_session_lifetime`.
- Pros: GC can be more aggressive (only holds back versions with live pins);
  scales to many concurrent readers without coordination.
- Cons: requires readers to write a pin object (one extra PUT before first read);
  pin expiry must be longer than the maximum observed reader session + clock skew;
  GC must handle the race where a pin is written concurrently with GC's check.
- Addresses Scenario A: yes (crashed reader's pin expires). Scenario B: depends on
  whether the pin is written before the first data read (it must be). Scenario C:
  yes (GC only deletes versions whose pins have expired).

**Option C — Generational manifest retention (retain N most recent versions)**

- How: GC never deletes the N most recent committed versions (e.g. N=5). Readers
  are expected to be reading one of the N most recent versions. If a reader is
  reading a version older than N, it may get 404s.
- Pros: simple; no per-reader objects; predictable storage overhead.
- Cons: on a busy writer with many small commits, N versions may be only a few
  seconds old — too short for slow readers. N must be sized for the burst commit
  rate * expected max session time, which is hard to size statically.
- Addresses Scenario A: partially (if N is large enough). Scenario B: yes.
  Scenario C: yes.

### Recommendation for SPIKE-0002 and SPIKE-0003

Use **Option A (retention grace window) as the primary mechanism**, with a stated
default of `gc_grace_seconds = 1800` (30 minutes), configurable by the operator.
This is the simplest design that covers all three scenarios without extra round trips
or per-reader writes, and is consistent with the no-coordinator architecture (R3
modes 2 and 3).

Add **Option B (TTL'd pin objects) as an optional extension** for deployments
where the grace window is too long (high-frequency write workloads with many old
versions accumulating). Pin objects are optional — their presence makes GC more
aggressive, but their absence still gives safety via the grace window.

The TLA+ model (SPIKE-0002) must model Option A as the primary mechanism (GC only
deletes versions older than `gc_grace`), and the invariant must hold for: a reader
that started at time T and reads an object from a version committed at time T-δ for
any δ < gc_grace.

### Precise discharge obligations for SPIKE-0002 and SPIKE-0003

**F3-P1 — Safe-GC policy specified in the format spec (SPIKE-0003):**

The format spec MUST include a "GC policy" section that states:
1. The `gc_grace_seconds` parameter and its default value.
2. The rule: "GC may only delete objects belonging to a version V if
   `now - V.commit_timestamp > gc_grace_seconds`."
3. How `V.commit_timestamp` is recorded (e.g. in the manifest object's metadata or
   in the manifest body) and how GC reads it.
4. The consequence for out-of-window readers: "a reader holding a version older
   than `gc_grace_seconds` may receive 404 errors; this is the reader's
   responsibility to handle by re-opening the database."

**F3-P2 — Scenario B prevention: read the manifest before writing any pin (SPIKE-0003 + SPIKE-0002):**

The protocol must define a "reader open" sequence that is: (a) resolve latest
manifest, (b) record the resolved version number + timestamp locally, (c) **if
using pin objects, write the pin before reading any data objects.** If not using
pin objects (Option A only), the reader must verify that
`now - manifest.commit_timestamp ≤ gc_grace_seconds` at open time, and reject
opens for versions too close to the grace boundary with a clear error rather than
silently reading and later getting 404s.

**F3-P3 — TLA+ GC-vs-reader interleaving invariant (SPIKE-0002):**

The TLA+ model must include a GC process that runs concurrently with readers and
deletes old version objects after the grace window expires. The invariant:

    INVARIANT \A reader \in active_readers :
        \A obj \in reader.pinned_version.objects :
            obj \notin gc_deleted_objects

must hold for all reachable states. The model must explicitly represent the grace
window as an abstract time counter and show the invariant holds when GC only deletes
versions with `commit_time < now - gc_grace`.

**F3-P4 — Scenario C (mid-GET delete) addressed by design note:**

The format spec must note: "S3 object deletes are issued only after the grace window
has expired. A reader mid-range-GET on an object that GC deletes will receive either
the full response (if the GET completed before the DELETE) or an error (if the DELETE
reached the object before the GET completed, which is only possible if the reader
has been running for longer than `gc_grace_seconds`). This is an accepted edge case
bounded by the grace window." The engine's reader implementation must handle
S3 404/error on a range GET gracefully (retry with re-open, or propagate a
recoverable error) rather than panicking.

**F3-P5 — Master-less mode explicit statement (SPIKE-0002):**

The commit-protocol ADR must include a "master-less mode (R3 mode 3)" section that
states: "GC runs only under the writer lease; a master-less DB (no active writer)
has no GC. Old versions accumulate until a writer attaches." This is not a gap — it
is intentional — but it must be stated explicitly so the TLA+ model does not include
a GC process in the master-less scenario. The model must cover: writer-present GC
(safe); master-less (no GC; reader always safe).

---

## Cross-reference: what must appear in each SPIKE's output

### SPIKE-0003 (storage format spec) must discharge

| Finding | Obligation | Section |
|---------|-----------|---------|
| F1 | F1-S1 adjacency chunking + `max_chunk_bytes` | Adjacency-list layout |
| F1 | F1-S2 chunk-index format + range-GET alignment | Adjacency-list layout |
| F1 | F1-S3 worst-case bytes-per-hop formula + value | Byte-budget analysis |
| F1 | F1-S4 LIMIT early termination as correctness obligation | Access pattern |
| F2 | F2-P1 name the exact conditional-PUT primitive | Commit mechanics |
| F2 | F2-P4 `supports_conditional_put()` precondition + fallback | Portability |
| F3 | F3-P1 GC policy section with `gc_grace_seconds` | Versioning and GC |
| F3 | F3-P2 reader-open sequence for Scenario B | Reader protocol |
| F3 | F3-P4 Scenario C design note | Versioning and GC |

### SPIKE-0002 (commit protocol ADR + TLA+ model) must discharge

| Finding | Obligation | Section |
|---------|-----------|---------|
| F2 | F2-P1 primitive named + request shape tabled | ADR: commit mechanics |
| F2 | F2-P2 mock-fidelity integration test specified | ADR: mock-fidelity verification |
| F2 | F2-P3 safety invariant "at most one commit per predecessor" | TLA+ model |
| F3 | F3-P3 GC-vs-reader interleaving invariant in TLA+ | TLA+ model |
| F3 | F3-P5 master-less mode GC statement | ADR: attach modes |
| F3 | F3-P2 reader-open sequence (pin timing / grace check) | ADR: reader protocol |

### SPIKE-0001 (latency cost model) must discharge

| Finding | Obligation | Section |
|---------|-----------|---------|
| F1 | F1-E1 realized fan-out assumption stated numerically | Cost model proof |
| F1 | F1-E2 cross-reference to SPIKE-0003 `max_chunk_bytes` | Cost model proof |

---

## License check (no external dependencies)

SPIKE-0008 is a constraint-documentation spike. It does not recommend any external
library, dataset, or tool. All constraints are dischargeable through protocol
design choices in SPIKE-0001, SPIKE-0002, and SPIKE-0003, with no new external
dependencies introduced. No license check required for this spike's output.

The related implementation tasks (T-0007, T-0008, T-0009, T-0010, T-0011, T-0012)
will use crates already in-scope for the project (e.g. `aws-sdk-s3` / `object_store`,
both Apache-2.0). Any new crate introduced by those tasks must pass the standard
`cargo deny check licenses` gate before landing.

---

## Open questions (none blocking this spike; noted for dependent work)

1. **Exact value of `max_chunk_bytes` (SPIKE-0003's choice):** the cost model
   requires this value to close the F1-E2 cross-reference. The two spikes must
   be co-authored or at minimum cross-checked. A value in the range 64 KB – 256 KB
   is consistent with both S3 GET economics (larger GETs are more efficient per
   byte) and the need to abort early without wasting too many bytes.

2. **`gc_grace_seconds` default value:** 1800 s (30 min) is proposed as the
   default. Workloads with very long-running analytical reads may need to set this
   higher. The engine should surface this as a named configuration key so it can be
   set per-database.

3. **Clock skew in the grace window:** the GC compares `now` (on the GC-running
   process's clock) with `V.commit_timestamp` (set by the writer at commit time,
   on the writer's clock). Clock skew between writer and GC process is bounded by
   NTP (typ. < 1 s); adding a 60 s slack to `gc_grace` (i.e. GC waits
   `gc_grace + 60 s`) is sufficient. The spec should mention this.

4. **moto conditional-write support:** the decision to use `If-None-Match: *` on
   new unique keys (Option A) was based on it being the most universally supported
   primitive. The mock-fidelity test (F2-P2) will confirm it. If moto does not
   support it on the version pinned in the Nix shell, the workaround is either:
   (a) upgrade the moto version in the Nix flake (MIT license, compatible), or
   (b) implement the uniquely-named-manifest-only resolution with no conditional
   header at all (write is idempotent for a unique name; race safety comes from the
   writer holding the lease when generating the name). Option (b) requires the
   fencing model to ensure only one writer can generate a given name — document the
   constraint in the ADR.

---

## Confidence and remaining gaps

**Confidence: high** that F1, F2, and F3 are now specified precisely enough for
SPIKE-0002 and SPIKE-0003 authors to act without further research. The options
analysis is grounded in the physics of the byte budget (F1), published S3 API
behavior (F2), and standard object-store GC patterns (F3).

**Remaining gap:** the exact numeric values (chunk size, grace window, realized
fan-out bound) are SPIKE-0003 and SPIKE-0001 design choices, not this spike's
output. This spike names the constraints and the cross-checks; the numeric values
are owned by the respective design spikes.

---

## Next steps

- Set SPIKE-0008 `status: done`.
- SPIKE-0003 author: implement obligations F1-S1 through F1-S4, F2-P1, F2-P4,
  F3-P1, F3-P2, F3-P4 in the storage format spec.
- SPIKE-0002 author: implement obligations F2-P1 through F2-P4, F3-P3, F3-P5,
  F3-P2 in the commit-protocol ADR + TLA+ model.
- SPIKE-0001 author: implement F1-E1 and F1-E2 in the cost-model proof.
- `steering-storage` closes SPIKE-0008 once F1, F2, F3 are marked discharged in
  the ratified SPIKE-0002 and SPIKE-0003 outputs.
- Implementation tasks T-0007, T-0008, T-0009, T-0010, T-0011, T-0012 must cite
  the discharge in their acceptance criteria before `steering-storage` ratifies
  SPIKE-0003. They remain `backlog` until then.
