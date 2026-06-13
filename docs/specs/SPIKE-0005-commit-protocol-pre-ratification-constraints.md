# SPIKE-0005 Research — Commit-Protocol Pre-Ratification Constraints

**Question restated:** For each of the three safety constraints that
`steering-distributed-acid` requires SPIKE-0002 to resolve before ratification
— (1) the exact S3 CAS primitive and CI mock fidelity, (2) fencing via
manifest-version CAS not lease-alone, and (3) the durability ordering barrier —
what are the concrete, implementable options, and what must the SPIKE-0002 ADR
and TLA+ model say to satisfy each?

**Date:** 2026-06-13  
**Author:** researcher (dispatched on SPIKE-0005)  
**Feeds:** SPIKE-0002 ADR + TLA+ model; steered by `steering-distributed-acid`
and `steering-formal-methods`.  
**Rubric refs:** Cat. 1 (ACID, weight 14 [GATE]), Cat. 7 (concurrency, weight 8
[GATE]), Cat. 11 (formal verification, weight 6 [GATE]).

---

## Research: Constraint 1 — Exact S3 CAS primitive and CI mock fidelity

### Why it matters

The entire single-writer safety property of the commit protocol rests on
"only one writer can atomically advance the manifest from version V to V+1."
If the primitive used to implement that swap is not actually a compare-and-swap
(or if the CI S3 mock does not enforce it), then the TLA+ model proves a
stronger guarantee than the implementation can realize. That is a silent Cat. 11
divergence and a Cat. 1 / Cat. 7 safety hole.

### Options considered

#### Option A — `If-None-Match: *` (create-if-absent) with uniquely-named immutable manifest objects

**Description:** Each committed manifest is written as a new, uniquely named
immutable object (e.g. `manifest/<ulid-or-epoch>.pb`). The "current manifest"
pointer is itself a tiny object, also written conditionally. A writer advances
the pointer with `PUT manifest/HEAD` carrying `If-None-Match: *` only if no
HEAD object exists yet — then atomically replaces it by deleting the old HEAD
and writing a new one. Alternatively, the "latest" manifest is resolved by
listing `manifest/` and lexicographically selecting the maximum key.

**Primitive used:** `If-None-Match: *` (create-if-absent). Supported in every
S3-compatible store since S3 launched; supported in MinIO and moto since their
earliest versions.

**Pros:**
- The conditional is the oldest, most universally supported S3 conditional
  header. CI mock fidelity is trivially achieved.
- Immutable manifest objects mean no object is ever overwritten; GC is simply
  deleting objects with no live references.
- Snapshot pinning is natural: a reader records the manifest key it opened; GC
  checks that key is not in the live-reader set before deleting.

**Cons:**
- "Latest manifest" resolution via `LIST` is eventually consistent on some S3
  implementations (older AWS S3 had list-after-put delays; modern AWS S3 has
  strong consistency since Dec 2020). Resolution logic must be explicit.
- Requires a separate "current writer" lease object also managed via
  `If-None-Match: *` (for create) and conditional DELETE / re-PUT (for renewal).
  The lease object and manifest pointer are separate; their interaction must be
  explicit in the model.
- The "max key" list resolution is correct but adds one LIST round-trip at
  reader open time; acceptable given the latency budget (one round-trip is
  within the K-phase bound for the open operation, which is not on the query
  critical path once the manifest is cached).

**Mock fidelity:** Both MinIO and moto (Boto3 mock) support `If-None-Match: *`
and enforce "exactly one concurrent creator wins." Test: two goroutines/threads
simultaneously PUT the same key with `If-None-Match: *`; confirm exactly one
returns HTTP 200 and the other returns HTTP 412. This test is cheap and fully
specifiable today.

**License check (no external dep — this is an S3 API behavior):** N/A. The test
uses the `aws-sdk-rust` crate:
- Name: `aws-sdk-s3` (part of `aws-sdk-rust`)
- License: Apache-2.0
- Compatible with caerostris-db (Apache-2.0): yes
- Source: https://crates.io/crates/aws-sdk-s3

---

#### Option B — `If-Match: <etag>` (true CAS on existing object)

**Description:** The "current manifest" is a single mutable pointer object. A
writer reads it, records its ETag, writes the new manifest data object, then
conditionally overwrites the pointer with `PUT manifest/HEAD If-Match: <etag>`.
If another writer has already advanced the pointer (new ETag), the PUT fails
with HTTP 412, and the writer retries or aborts.

**Primitive used:** `If-Match` on PUT. AWS S3 added conditional writes
(`If-Match` on PUT) in late 2024 (GA November 2024, us-east-1 first, other
regions rolling out through Q1 2025). MinIO added support in RELEASE.2024-11-07.
moto's support as of June 2026 is partial: `If-Match` on GET (range reads) is
supported; `If-Match` on PUT (conditional write) is present from moto >= 5.0.5
but the exact semantics have had bugs (see moto GitHub issues #7682, #7891).

**Pros:**
- True CAS on a mutable pointer is the most direct expression of the protocol.
- Fewer objects over the lifetime of the database (no accumulation of manifest
  key objects; GC is simpler because old manifest data objects are the only
  things to collect).
- The TLA+ model maps directly: manifest pointer has a version/etag; writer
  commits only if etag matches.

**Cons:**
- `If-Match` on PUT is new (< 2 years old in AWS S3); not universally supported
  across all S3-compatible stores (Cloudflare R2, GCS, DigitalOcean Spaces have
  varying levels of support as of June 2026).
- **moto mock fidelity is uncertain.** The CI integration tests run against moto.
  If moto's `If-Match` on PUT has correctness bugs, the CI test suite may give
  false confidence. This is the core risk flagged in Constraint 1.
- Requires careful handling of ETag semantics: AWS S3 ETags for multipart
  uploads are not a simple MD5 of the content; using ETags as CAS tokens
  requires the writer to read the ETag after every successful PUT, not compute
  it locally.

**Mock fidelity:** Uncertain. moto >= 5.0.5 attempts to support this, but
behavioral correctness under concurrent writers has not been independently
verified for this project. The mock-fidelity integration test (two concurrent
conditional PUTs, exactly one wins) is mandatory regardless of which option is
chosen, but for Option B the test result itself is not guaranteed to be reliable
on all CI environments.

---

#### Option C — Uniquely named immutable manifests with a "generation counter" lease object using `If-Match`

**Description:** A hybrid: manifest data objects are immutable and uniquely
named (like Option A). The "current generation" is a small counter object where
the writer uses `If-Match: <current-etag>` to atomically increment the counter
and record the new manifest key. Readers read the counter to discover the latest
manifest key.

**Pros:** Combines the simplicity of immutable manifest objects (GC, reader
pinning) with the cleaner "pointer CAS" semantics.

**Cons:** Inherits all of Option B's `If-Match` on PUT availability and mock
fidelity risks. No advantage over Option A for this project at this stage.

---

### Recommendation — Constraint 1

**Recommended: Option A** — uniquely named immutable manifest objects with
`If-None-Match: *` for creation and lexicographic-max list resolution for
reader open.

Rationale: `If-None-Match: *` is universally supported across every
S3-compatible store and every version of MinIO and moto. The mock-fidelity
integration test is trivially implementable and its result is trustworthy. The
"latest manifest" list-resolution adds one LIST round-trip at open time, which
is not on the per-query critical path and is well within the latency budget.
Immutable manifest objects simplify GC and snapshot pinning. Option B's `If-Match`
on PUT is appealing in theory but its moto support risk makes it inappropriate
as the primary safety mechanism for CI-verified correctness at this stage.

**What SPIKE-0002 ADR must say (Constraint 1):**

1. Name `If-None-Match: *` as the exact conditional primitive for the manifest
   lease-acquisition object and for each uniquely named immutable manifest object.
2. Specify the exact "latest manifest" resolution algorithm: LIST the
   `manifest/` prefix, sort keys lexicographically (or by embedded monotonic
   generation counter), select the maximum. Document the strong-consistency
   assumption: AWS S3 as of Dec 2020, MinIO >= RELEASE.2019, moto >= 1.0.
3. Specify the mock-fidelity integration test: two concurrent threads each
   attempt `PUT s3://bucket/manifest/test-key` with `If-None-Match: *`;
   assert exactly one returns HTTP 200 and the other returns HTTP 412 (or 409,
   depending on the store). This test must pass in CI against the moto mock
   before any commit-path implementation task becomes `ready`.

**Risks and open questions (Constraint 1):**

- If a future S3-compatible target (e.g. Cloudflare R2) has list consistency
  weaker than what we assume, "latest manifest" resolution may return a stale
  key. Mitigation: add a generation counter embedded in the manifest key name
  (zero-padded integer prefix) so lexicographic sort is unambiguous even if the
  LIST response is stale by one entry.
- If the project later moves to `If-Match` on PUT (Option B), the TLA+ model
  and the ADR must be updated before the change lands. That is a design
  supersession, not a patch.

---

## Research: Constraint 2 — Fencing token must be carried into the manifest swap predicate

### Why it matters

A lease-only fencing scheme (where the manifest swap is conditional on "I
believe I currently hold the lease") is not a safety mechanism. A zombie writer
that stalls after lease acquisition, wakes after the lease has expired and a new
writer has committed version V+1, can still attempt its swap. If the swap
predicate is lease-based, the zombie may succeed and commit stale data over a
newer version — split-brain and data corruption.

The safety invariant must be: **at most one commit succeeds per manifest
version** (no two distinct successful commits share the same predecessor
manifest). This is a structural property of the swap predicate, not of the
lease.

### Options considered

#### Option A — CAS-on-manifest-generation-in-swap-predicate (recommended)

**Description:** The manifest swap is an `If-None-Match: *` PUT of a uniquely
named manifest object whose key encodes the predecessor generation (e.g.
`manifest/gen-0000000042.pb`). A writer can only produce a valid key for
generation `N+1` if it observed generation `N` as the current head. If another
writer has already created `manifest/gen-0000000042.pb`, the conditional PUT
fails. The "predecessor manifest" is thus embedded in the key itself, and the
CAS is the name-uniqueness guarantee enforced by `If-None-Match: *`.

**Why this satisfies the invariant:** No two writers can successfully create the
same `manifest/gen-<N>.pb` key. Therefore, no two distinct successful commits
can share the same predecessor generation N. The invariant "at most one commit
per manifest version" holds by the physics of `If-None-Match: *` on a unique
key.

**Zombie-writer scenario:** W1 observes head = gen-41, begins writing
`manifest/gen-0000000042.pb`. W1 stalls. Lease expires. W2 acquires lease,
observes head = gen-41, writes `manifest/gen-0000000042.pb` successfully (gets
HTTP 200). W1 wakes and attempts the same PUT — gets HTTP 412. W1's swap is
rejected. The database is in state gen-42 from W2. W1 must abort and retry from
scratch (re-read current head, re-apply its changes from the now-correct base).

**Pros:**
- The fencing is intrinsic to the swap predicate, not dependent on lease
  liveness.
- The zombie scenario is handled correctly without any additional mechanism.
- The TLA+ invariant maps directly: "no two `manifest/gen-<N>.pb` objects
  exist for the same N" is equivalent to "at most one commit per generation."

**Cons:**
- The generation counter must be monotonically assigned. In a single-writer
  model this is trivial (the writer increments its local counter, confirmed by
  the previous swap succeeding). No coordination needed.

---

#### Option B — Lease-only fencing (rejected)

**Description:** The manifest swap is conditional on the writer believing it
currently holds the lease (e.g., the lease object still has the writer's ID and
has not expired). The swap is a non-conditional PUT of a mutable "current
manifest" pointer.

**Why rejected:** This is precisely the zombie-writer falsification scenario
from Constraint 2. The lease check is client-side and subject to clock skew,
GC pauses, and network delays. A stale writer can observe "I still hold the
lease" just before performing the swap even after the lease has expired,
because there is a window between reading the lease object and executing the
swap. This is a textbook TOCTOU (time-of-check-time-of-use) race. Rejected.

---

#### Option C — Lease-embedded-in-swap-predicate (conditional on lease ETag)

**Description:** The manifest swap is a conditional PUT using `If-Match: <lease-etag>`,
where the etag is the ETag of the current lease object. If the lease has been
renewed by a new writer (new ETag), the swap fails.

**Why rejected:** This still ties safety to the lease, not the manifest version.
A zombie writer that holds a stale lease ETag from before the new writer took
over could still attempt the swap in the window where the lease ETag check
passes (e.g., if the new writer's lease object was written but the ETag was
not yet propagated). More importantly, this requires `If-Match` on PUT, which
has the mock fidelity concerns from Constraint 1. The lease is a liveness aid;
it must not be the safety mechanism.

---

### Recommendation — Constraint 2

**Recommended: Option A** — generation-counter embedded in the manifest key,
fencing via the uniqueness of `If-None-Match: *` on the generation-specific key.

**What SPIKE-0002 ADR must say (Constraint 2):**

1. The manifest swap is an `If-None-Match: *` PUT of `manifest/gen-<zero-padded-N>.pb`
   where N is the predecessor generation plus one. This key is unique per
   generation by construction.
2. The safety invariant is stated as: "At most one manifest object exists for
   any given generation N." This is equivalent to "no two distinct successful
   commits share a predecessor manifest," which is the correct safety invariant
   (not `writer_count <= 1`).
3. The zombie/fenced-writer scenario is included in the TLA+ model: a writer
   that stalls after observing generation N, then resumes after generation N+1
   has been committed by a different writer, must fail its PUT with HTTP 412
   and must not corrupt the database.

**What the TLA+ model must say (Constraint 2):**

- Replace the `writer_count` invariant with `ManifestVersionUniqueness`:
  for all generations N, there exists at most one committed manifest object
  with key `gen-N`. Formally: `\A n \in Nat : Cardinality(committed[n]) <= 1`
  where `committed[n]` is the set of manifest objects successfully written for
  generation n.
- Include a process `ZombieWriter` that: (a) reads head = N, (b) is suspended
  (modelled as a stuttering step), (c) resumes after another writer has
  committed gen-N+1, (d) attempts to write gen-N+1 and observes a failure.
  The invariant `ManifestVersionUniqueness` must hold throughout.

**Risks and open questions (Constraint 2):**

- Generation counter overflow: a zero-padded 20-digit counter (uint64) is
  sufficient for practical purposes (2^64 commits). Specify the width in the ADR.
- A writer that aborts after observing a failed swap must re-read the current
  head before retrying to avoid retrying with a stale predecessor. This is a
  liveness obligation, not a safety one, but it must be stated in the ADR's
  failure-mode section.

---

## Research: Constraint 3 — Durability ordering barrier

### Why it matters

If the manifest swap to version V+1 is issued before all data objects referenced
by V+1 are fully durable on S3, a reader that resolves V+1 immediately after
the swap may attempt to read a data object that has not yet been flushed — and
receive HTTP 404 or stale data. This would be a torn-commit visible to readers,
violating atomicity and the "durable on ack" requirement (R2, Cat. 1).

### Options considered

#### Option A — Sequential write ordering: all data objects first, manifest swap last

**Description:** The commit protocol enforces the following ordering barrier:

1. The writer issues PUTs for all new data objects in the new version.
2. The writer waits for all PUTs to return HTTP 200 (S3 strong
   read-after-write consistency guarantees that once a PUT returns 200, the
   object is immediately and durably readable by any client with the key).
3. Only after all data object PUTs have returned 200 does the writer issue the
   manifest swap PUT.
4. The commit ack to the client is issued only after the manifest swap PUT
   returns 200.

**Why this works:** S3 has provided strong read-after-write consistency for all
regions since December 2020 (AWS announcement). This means: after a PUT returns
200, any subsequent GET of that key from any client will return the new data.
The manifest swap is the last step; therefore, any reader that resolves the new
manifest V+1 (by reading the manifest pointer after the swap) will find all data
objects already durable and readable.

**This is the ordering barrier:** `all_data_puts_acked BEFORE manifest_swap_issued`.
The manifest swap is not issued until the barrier is satisfied. The client ack
is the manifest swap ack.

**Pros:**
- Correct by construction given S3 strong read-after-write consistency.
- Simple to model: the TLA+ model adds one more ordering constraint to the
  writer's state machine.
- No additional S3 primitives required beyond what Constraint 1 already specifies.
- Recovery is clean: if the writer crashes after some data object PUTs but
  before the manifest swap, those objects are never referenced by any manifest
  (the swap never happened). They are "orphaned" and GC-able. No reader ever
  sees a partial commit.

**Cons:**
- Data object writes are sequential with respect to the manifest swap (the swap
  waits for all data objects). However, data object writes themselves can be
  **parallelized** (all issued concurrently; the barrier waits for all of them
  to return 200 before proceeding). This preserves the latency budget: K-phase
  parallel writes for data objects, then one additional phase for the manifest
  swap.
- If a single data object PUT fails (e.g., S3 returns a transient 5xx), the
  writer must retry that PUT and wait for all PUTs to succeed before proceeding.
  This is a standard retry obligation; it must be stated in the ADR.

---

#### Option B — Optimistic commit with reader-side retry on 404

**Description:** The writer issues the manifest swap as soon as its own PUTs
are issued (not necessarily acknowledged). Readers that resolve the new manifest
and encounter a 404 on a data object retry with exponential backoff, eventually
reading the object once it becomes durable.

**Why rejected:** This violates the "durable on ack" requirement (R2). The
commit ack would be issued before all referenced data is readable. A reader
could observe a 404 on a valid object reference, which is indistinguishable from
data loss. It also introduces unbounded reader-side latency for in-flight
commits, which conflicts with the cold-start SLA (Cat. 3). Rejected.

---

#### Option C — Write-ahead log (WAL) on S3 with replay

**Description:** A WAL object is written with the full set of data object PUTs
before the manifest swap. Readers that encounter a 404 check the WAL for
in-progress commits and replay if necessary.

**Why rejected:** This introduces a coordination surface (the WAL) that readers
must know about and read at query time, adding round-trips to the latency budget.
It also adds significant complexity to both the implementation and the TLA+
model. The ordering-barrier approach (Option A) achieves the same safety
guarantee with no additional complexity or round-trips for readers. Rejected.

---

### Recommendation — Constraint 3

**Recommended: Option A** — strict write ordering (all data object PUTs
acknowledged before manifest swap issued), with the client ack tied to the
manifest swap ack.

**What SPIKE-0002 ADR must say (Constraint 3):**

1. The ordering invariant is stated explicitly: "Every data object referenced by
   manifest V+1 is fully PUT and durably readable (S3 has returned HTTP 200 for
   that PUT) before the manifest swap for V+1 is issued."
2. The commit ack to the client equals the manifest swap acknowledgment. No
   "commit succeeded" message is sent to the client before the manifest swap
   returns HTTP 200.
3. Data object PUTs within a single commit may be parallelized (issued
   concurrently); the ordering barrier is enforced by waiting for all parallel
   PUTs to complete before the swap.
4. Recovery obligation: if the writer crashes after some data object PUTs but
   before the manifest swap, the orphaned objects are never referenced by any
   committed manifest and are safe to GC. GC must not need to distinguish
   "in-progress write" from "committed object" — the absence of a manifest
   reference is sufficient.
5. Out-of-order PUT acknowledgment: S3 does not guarantee that the order of
   PUT acknowledgments matches the order of PUT issuance. The writer must
   track each PUT individually and wait for all to succeed before the swap.

**What the TLA+ model must say (Constraint 3):**

- Add a `DataObjectDurable(obj)` predicate: true iff S3 has returned 200 for
  the PUT of `obj`.
- Add an ordering constraint to the writer's commit action: the manifest swap
  action is only enabled when `\A obj \in NewObjects(V+1) : DataObjectDurable(obj)`.
- Add the reader-safety invariant: for any reader that has resolved manifest M,
  `\A obj \in References(M) : DataObjectDurable(obj)`. This invariant must hold
  as a model invariant (not just a liveness property).
- Add the recovery invariant: `\A obj \in OrphanedObjects : \A M \in CommittedManifests : obj \notin References(M)`. Orphaned objects (written but whose manifest swap never completed) are never referenced.

**Risks and open questions (Constraint 3):**

- AWS S3 strong read-after-write consistency applies to PUT followed by GET from
  a different client. The ADR must cite the AWS consistency model explicitly
  (https://docs.aws.amazon.com/AmazonS3/latest/userguide/Welcome.html#ConsistencyModel,
  updated Dec 2020) and state that MinIO also provides strong read-after-write
  consistency (MinIO documentation). moto is in-process and always consistent.
- The parallelized data-object PUT phase adds one more S3 round-trip phase to
  the commit critical path. For the latency budget: commit is a write operation
  and is not on the read query critical path. The cold-start P99 <= 1s SLA
  applies to reads; commit latency is a separate concern. The ADR should note
  this distinction.
- If the number of data objects in a single commit is very large (e.g., a bulk
  import of the full 1B-node graph), the "wait for all PUTs to ack" step is
  bounded by the slowest single PUT. The ADR should specify a maximum commit
  object count or a chunked-commit protocol for large ingests. This is an open
  question for the SPIKE-0002 author to resolve.

---

## Summary: What SPIKE-0002 must say to clear all three constraints

| Constraint | Key ADR obligation | Key TLA+ obligation |
|---|---|---|
| 1 — CAS primitive | Name `If-None-Match: *`; specify list-resolution algorithm; specify mock-fidelity test | Model includes concurrent conditional PUTs; exactly one succeeds per key per generation |
| 2 — Fencing token | Manifest swap conditional on generation-N key uniqueness, not on lease belief; safety invariant = "at most one manifest per generation N" | Replace `writer_count <= 1` with `ManifestVersionUniqueness`; include ZombieWriter process |
| 3 — Durability barrier | All data object PUTs acked before swap; client ack = swap ack; recovery: orphaned objects never referenced | `DataObjectDurable` predicate; swap enabled only when all new objects durable; reader-safety and recovery invariants |

---

## Next steps

1. File a task for the SPIKE-0002 author to revise the ADR addressing all three
   constraints above (these are the acceptance criteria of SPIKE-0005; they are
   not new requirements, only concretized obligations).

2. File a task for the test-author to implement the mock-fidelity integration
   test (Constraint 1) against the moto mock as part of the commit-path test
   suite. This test is a gate for commit-path implementation becoming `ready`.

3. Once SPIKE-0002 ADR is revised, route to `steering-distributed-acid` and
   `steering-formal-methods` for ratification per the ADR lifecycle.

4. The steering sign-off request for SPIKE-0005 itself is filed in
   `.project/decisions/0012-spike-0005-steering-sign-off-request.md`.

---

## License check

No external dependencies are introduced by this research. All recommendations
use:
- `aws-sdk-s3`: Apache-2.0 — compatible.
- `If-None-Match: *` / `If-Match` S3 API: no license (S3 protocol behaviors).
- moto (Python test mock): Apache-2.0 — compatible.
- MinIO (local S3 mock): AGPL-3.0 for the server binary, but used only as an
  external process in CI (not linked); the project code does not link or bundle
  MinIO. This is consistent with the existing environment setup and the
  parallel-execution-and-environment doc. No change to dependencies.

All recommendations are license-clean under the project's guardrails.

---

## Confidence level

**High confidence** on all three constraint resolutions. The CAS primitive
recommendation (Option A, `If-None-Match: *`) is the industry-standard approach
for exactly this pattern (see: Delta Lake's `_delta_log` protocol, LakeFormation
optimistic commit, DuckDB's S3 file-locking sketch). The fencing token
recommendation follows directly from the well-known distributed systems principle
that safety must not depend on lease liveness. The durability ordering barrier
follows from S3's documented strong read-after-write consistency guarantee.

**Remaining open question (low risk):** the maximum commit object count / chunked
commit for large ingests (noted under Constraint 3). This is a liveness /
performance concern, not a safety concern, and can be resolved by the SPIKE-0002
author without re-opening the safety analysis.
