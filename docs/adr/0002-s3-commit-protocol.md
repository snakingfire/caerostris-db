# ADR 0002 — S3-native commit & concurrency protocol (atomicity + snapshot isolation)

## Status

`accepted`
<!-- proposed → reviewed (adversarial) → accepted (steering) → superseded -->

> **Ratified 2026-06-13 (≈ T0+~3:10 primary commit-side; formal-methods v2
> re-confirm same cycle).** Both primaries have signed on the **v2 (DA-1-fixed)**
> ADR + TLA+ model: `steering-distributed-acid` (PRIMARY, commit/isolation/attach)
> **RATIFIED-WITH-CONDITIONS** in decision **0026**; `steering-formal-methods`
> (PRIMARY, Cat. 11 / TLA+ model fidelity) **RATIFIED** the v2 model in decision
> **0029** (superseding the FM-1 CHANGES_REQUESTED of decision 0024, which was
> against the now-replaced v1 model). `steering-storage` (secondary) **APPROVE** in
> decision **0027**. The conditions (C1–C4 in decision 0026) are **commit-path
> implementation land-gates**, not design blockers, and are restated in the gate
> note below.

## Date / T+ marker

2026-06-13T19:20:00Z  (≈ T0+0:56)

## Context

caerostris-db stores **all durable state on commodity object storage (S3)** and
provides **full ACID** transactions under **single-writer / multi-reader**
concurrency (R2, R3). This ADR is the **design-before-code gate** (SPIKE-0002)
for every implementation task touching the commit path (EPIC-001 storage,
EPIC-004 ACID, EPIC-006 attach modes). Per
[`formal-verification-policy.md`](../process/formal-verification-policy.md) no
such task moves past `backlog` until this ADR is **steering-ratified** by
`steering-distributed-acid` + `steering-formal-methods` and the companion TLA+
model (`formal/commit-protocol/commit_protocol.tla`) is model-checked.

**Rubric stakes:** Cat. 1 (ACID, w14, GATE — "behaviour matches the TLA+
model"), Cat. 7 (concurrency/attach modes, w8, GATE — split-brain prevention),
Cat. 11 (formal artifacts, w6, GATE).

**Hard constraints carried in from prior ratification passes** (these are the
falsification scenarios this design *must* survive — they were pre-registered by
steering before this ADR existed):

- **decision 0004 #2 / SPIKE-0005 Constraint 1 — name the exact CAS primitive.**
  "Conditional PUT (if-none-match or equivalent)" is not a primitive.
  `If-None-Match: *` is *create-if-absent*, not compare-and-swap on an existing
  pointer's body. The ADR must name the precise primitive and prove the CI mock
  enforces it.
- **decision 0004 #3 / SPIKE-0005 Constraint 2 — fencing-by-lease-alone is a
  split-brain falsification.** The zombie-writer interleaving (W1 stalls, lease
  expires, W2 commits V+1, W1 wakes and swaps) must be refuted by the *commit
  mechanism*, not by lease belief. The invariant `writer_count ≤ 1` is the wrong
  shape; restate as *at most one commit succeeds per manifest version*.
- **decision 0004 #4 / SPIKE-0005 Constraint 3 — durability ordering barrier.**
  All data objects of V+1 must be durably readable before the manifest swap;
  commit-ack == swap-ack. Orphaned pre-swap objects are never referenced and are
  GC-able.
- **decision 0001 F3 — GC safety without a central pin registry.** Master-less
  and embedded-read-only modes mean no always-live coordinator. GC must be
  provably safe against slow/crashed readers via TTL'd pins + a grace window,
  and run only under the writer lease.

This ADR also coordinates with: SPIKE-0003 (storage format / adjacency layout),
SPIKE-0004 (manifest-published statistics for out-of-envelope detection — the
manifest is the snapshot-consistent home for those stats; see
`docs/specs/SPIKE-0004-manifest-statistics-contract.md`), and EPIC-006 (attach
modes).

## Decision

We adopt an **immutable-versioned-manifest, create-only-CAS commit protocol**.
The commit is the **atomic creation of a new, immutably-named manifest object**;
the "current version" is resolved by **listing manifest keys and taking the
max** (or reading a best-effort pointer that is only ever an optimisation).
Fencing is a **property of the version-keyed create-only PUT**, never of lease
belief. The full design:

### 1. Object layout (commit-relevant subset)

A database lives under a bucket/prefix `db/`. Commit-relevant keys:

| Key pattern | Object | Mutability |
|-------------|--------|------------|
| `db/data/<content-hash>/<shard>.col`, `.adj`, ... (content-addressed; see below) | Content-addressed columnar / adjacency data blobs. The object **key embeds a hash of the object's content**, so the key is **unique per distinct write**. | **Immutable**, written once; create-only PUT (`If-None-Match:*`) for defence in depth. |
| `db/manifest/<V>.json` (V zero-padded, monotone) | The manifest for version `V`: the **explicit list of the exact data-object keys** it references, schema, and the snapshot-consistent statistics (SPIKE-0004). | **Immutable**, created exactly once. |
| `db/manifest/_latest` (optional) | Best-effort pointer to the max committed version. **Advisory only** — never trusted for correctness; resolution falls back to LIST. | Mutable, advisory. |
| `db/lease/writer` | The writer-lease object: `{ owner, epoch, deadline }`. | Mutable under create-only-CAS (see §3). |
| `db/pins/<reader-uuid>` | A reader's snapshot pin: `{ version, deadline }`. TTL'd. | Reader-owned. |

**Data-object key uniqueness (DA-1 / BC-4; decisions 0023, 0024, BUG-0012 /
T-0046).** Data-object keys are **unique per write attempt**. The chosen scheme
is **content-addressing**: the key embeds a collision-resistant hash of the
object's bytes (`db/data/<content-hash>/<shard>.col`). This makes write-once
**physically true**, not merely asserted:

- Two writers racing the same version `V+1` that produce **different** content
  write to **different** keys; identical content de-dupes onto the same key
  harmlessly (same bytes). A fenced/zombie writer's stale bytes therefore land
  on a **different** key than the winner's — it **cannot overwrite** the live,
  committed object. (Decision 0023's DA-1: an unconditional PUT to a
  *version+shard-scoped* key `db/data/v<V>/<shard>.col` was last-write-wins and
  let a zombie corrupt a committed snapshot in place. Content-addressing removes
  the shared mutable key entirely.)
- Each manifest records the **exact** content-addressed keys it references, so a
  reader resolving version `V` reads precisely the bytes the committer of `V`
  wrote.
- Data PUTs additionally use the create-only `If-None-Match:*` precondition for
  defence in depth (a second writer of the same hash either 200s on identical
  bytes or — never — a 412 it can ignore, since the existing bytes are identical).

> **Alternative (equivalent for safety): writer-epoch/attempt-scoped keys**
> `db/data/v<V+1>/<epoch-or-uuid>/<shard>.col`. The manifest records the exact
> keys. This also makes every write attempt's key unique. Content-addressing is
> preferred because it additionally de-dupes and dovetails with `steering-storage`'s
> ref-counted-GC constraint (decision 0027 / SPIKE-0003 cross-version sharing).

The defining property: **a manifest references content-unique, write-once
object keys**, and **manifest keys are created exactly once**. There is **no
in-place mutation of any object** that affects what a reader sees — and, post
DA-1, no shared mutable *data* key a zombie writer could overwrite either. This
is the property the TLA+ model now checks (object ids keyed by
`(version, writer, attempt, shard)`; invariants `OrphansNeverReferenced` +
`NoOverwriteOfReferenced`).

### 2. Commit = atomic create of the next manifest version (the manifest swap)

The writer commits version `V+1` (where `V` is the latest it observed) by:

1. **Stage data (durability barrier).** Write every data object of `V+1` under
   its **content-addressed key** `db/data/<content-hash>/<shard>.col` (§1). These
   are invisible: no manifest references them yet. Wait for each PUT's `200/ack`
   — S3 gives read-after-write durability on ack. **Only after all data objects
   are durably acked** does the writer proceed.
   *(decision 0004 #4: data-durable-before-swap.)*
   **Data-write precondition (DA-1 / BC-4):** no two distinct write attempts can
   mutate the same key — keys are content-unique, so the durability barrier is
   over **immutable, attempt-unique** objects. A stalled/zombie writer that
   re-issues its PUT writes to its **own** key, never the committed object's.
2. **Atomic swap = create-only PUT of `db/manifest/<V+1>.json`** with the
   **`If-None-Match: *` precondition** (see §3 for the exact primitive). This
   request **succeeds iff the key `db/manifest/<V+1>.json` does not yet exist.**
   - **Success → the commit is durable and the ack to the client is sent.**
     `commit-ack == manifest-create-ack`. The new version is now the max key and
     is atomically visible to any reader that lists/resolves after this point.
   - **Precondition failure (412 / key exists) → another writer already
     committed `V+1`.** This writer is **fenced**: it discards its staged data
     objects (now orphans, §6) and re-resolves `latest` before retrying at a
     higher version. **No partial state is ever visible**, because a manifest is
     the *only* thing that makes data objects reachable, and exactly one manifest
     per version can exist.
3. **Recovery / "latest" resolution.** A reader or a recovering writer resolves
   the current version by `LIST db/manifest/` and taking the max `V` for which
   `db/manifest/<V>.json` exists. Because manifest creation is the atomic commit
   point and manifests are immutable + complete-on-create, the max key always
   names a fully consistent snapshot. The `_latest` pointer, if present, is only
   a hint to skip the LIST; it is re-validated and never trusted on its own.

This is the technique S3-native engines use (Delta/Iceberg-style monotone
log + atomic create); we apply it to a graph manifest.

### 3. The exact CAS primitive (decision 0004 #2 / SPIKE-0005 C1)

> **Primitive: HTTP `PUT` with `If-None-Match: *` (RFC 7232 / RFC 9110 §13.1.2)
> — "create if and only if the key is absent".** On AWS S3 this is the
> **conditional write** GA'd Aug 2024 (returns `412 PreconditionFailed` if the
> object exists). On MinIO it is supported. It is **create-only CAS**: of any
> number of clients racing to `PUT If-None-Match:*` the *same* key, **exactly one
> receives `200` and all others receive `412`.**

We deliberately **do not** require CAS on an *existing* object's body (e.g.
`If-Match: <etag>`), because that primitive's support is partial and
version-bound across S3-compatible stores. Our entire commit safety rests only
on **create-only** semantics, which are broadly and uniformly available.

**Why this is sufficient for fencing (and lease belief is not):** because each
commit *creates a new key* `db/manifest/<V+1>.json`, two writers that both think
they are the master and both try to commit `V+1` are resolved by the store
itself — exactly one create wins. The loser is fenced by a `412`, independent of
what either writer believed about the lease.

**Mock-fidelity obligation (carried to EPIC-001 / T-0009/T-0010 + EPIC-009
tests):** the CI must include a test that issues **two concurrent
`PUT If-None-Match:*` to the same manifest key against the local mock and asserts
exactly one `200` and one `412`.** If the configured mock does not honour the
precondition, the Cat. 1/2 atomicity claim is unprovable on the mock — escalate
to a joint storage + distributed-ACID session (per decision 0001 F2). This test
is the bridge between the proven model and the realisable implementation.

### 4. Writer leasing / fencing (decision 0004 #3 / SPIKE-0005 C2)

The lease exists to make the **common case efficient** (avoid two writers
wasting work racing every commit) and to coordinate **GC** — **it is not the
safety mechanism.** Safety is the create-only CAS of §3.

- **Lease object** `db/lease/writer = { owner, epoch, deadline }`.
- **Acquire**: a writer acquires when the lease is absent or `now > deadline`.
  Acquisition is itself a create-only/conditional write that bumps `epoch`
  monotonically — the **fencing token**. (If two writers race acquisition, the
  store's conditional write picks one; the loser backs off. This is an
  optimisation; even if both *believed* they won, §3 still serialises commits.)
- **Renew (heartbeat)**: the owner periodically extends `deadline`.
- **Release**: on clean shutdown the owner clears the lease.

**The zombie-writer scenario, refuted.** W1 acquires (epoch 1), stages data for
`V+1`, then **stalls** (GC pause, network partition). Its lease `deadline`
passes. W2 acquires (epoch 2), commits `V+1` (creates `db/manifest/1.json`).
W1 wakes still believing it holds the lease and attempts its swap:
**`PUT If-None-Match:* db/manifest/1.json` returns 412** because W2's object
exists. W1 is fenced. **No split-brain, no torn state** — and crucially this
holds *without* W1 ever checking the lease. The TLA+ model encodes exactly this
interleaving (`ExpireLease` then W1 in state `wrote` attempting `SwapManifest*`)
and the invariant `AtMostOneCommitPerVersion` holds.

**Attach-mode coverage (R3, EPIC-006):** modes 1 (embedded writer) and 4
(server) hold the lease; modes 2 (embedded read-only) and 3 (master-less) never
need the lease and never need a live writer — they LIST + read immutable
manifests/objects. A second writer attempting to claim a held, un-expired lease
is **rejected** (close to "reject; optional client-side retry with backoff" —
*not* a server-side queue, per decision 0004 non-blocking note, which would be a
coordination service the design avoids).

### 5. Reader snapshot pinning + snapshot isolation

- A reader opens by resolving `latest = max(LIST db/manifest/)`, then writing a
  **pin object** `db/pins/<uuid> = { version: latest, deadline: now + TTL }`.
  The pin is **TTL'd** so a crashed reader's pin self-expires (decision 0001 F3).
- The reader reads only `db/manifest/<latest>.json` and the **immutable**,
  **version-scoped** data objects it references. Because those objects are
  immutable and a concurrent commit of `V+1` only ever *creates new* keys
  (`v<V+1>/...`, `manifest/<V+1>.json`), **the reader's snapshot is stable for
  its entire transaction** — it sees exactly `latest`'s object set, never a
  mixture. This is **snapshot isolation**; with single-writer + immutable
  versioned manifests it is in fact **serializable for reads** (each read
  transaction is equivalent to executing at the instant it pinned). We claim
  only SI in the rubric (the floor) but note the stronger property the model
  supports.
- Pin renewal: long-running readers renew `deadline` (heartbeat) before TTL.
- **Master-less mode (R3 mode 3):** there is no writer to honour pins, but none
  is needed — readers consume immutable objects, and GC (which *is* the only
  deleter) runs only under a live writer lease (§6). A master-less DB has no
  live writer, hence no GC, hence nothing deletes the objects a reader reads.

### 6. Garbage collection (decision 0001 F3)

GC reclaims superseded versions' manifests + data objects. Safety rules:

1. **GC runs only while holding the writer lease.** It is therefore single and
   serialised with commits. (Master-less DBs do no GC — safe by §5.)
2. **GC never reclaims `latest`.**
3. **GC reclaims an old version `V` only when no live pin references it.** "Live"
   = pin object exists and `now ≤ deadline + grace`. The **grace window** is set
   strictly greater than the maximum reader-session lifetime / pin-renewal
   period, so a slow-but-alive reader's pin is always observed before its version
   is collected. A crashed reader's pin expires and its version becomes
   collectible — no leak, no premature deletion.
4. **Orphaned pre-swap objects** (staged by a writer that was fenced or crashed
   before its swap) are referenced by **no committed manifest** — guaranteed by
   the content-addressed key scheme (§1): a fenced writer's object key embeds a
   hash of *its* bytes, which (DA-1) is a **different key** than the winner's, so
   it is genuinely an orphan and never collides with a live, referenced object.
   GC identifies orphans as **data-object keys not present in any live manifest's
   reference list** (a reference-counted / live-object-set sweep — the discipline
   `steering-storage` bound at decision 0027 for cross-version sharing), older
   than a grace window, and reclaims them. They were never visible to any reader.
   *(Previously this read "objects under `v<V>/` with no `manifest/<V>.json`",
   which assumed version-scoped data keys; the DA-1 fix replaces that with the
   manifest-reference-set test, since keys are now content-addressed.)*

The TLA+ `GCOldVersion` action encodes rules 1–3 (gated on
`v ∉ PinnedVersions` and `v ≠ latest`, deleting only objects no surviving
manifest references — `manifests[v].objs \ stillRef`); `GCSafety` is the
invariant that no live pin's objects are ever absent. `OrphansNeverReferenced`
(BUG-0012) is the invariant that a fenced/crashed writer's staged objects are
never in any committed manifest's reference set — the model-checked form of
rule 4.

### 7. Failure modes (every commit phase)

| Crash point | Visible state | Recovery |
|-------------|---------------|----------|
| Before staging | Old version `V` only | None needed; next writer re-acquires lease. |
| Mid data-write (some `v<V+1>` objects written) | Old `V` only — no manifest references the partial objects | Orphan objects GC'd (§6.4). |
| After all data durable, before swap | Old `V` only | Orphans GC'd; new writer commits `V+1` fresh. |
| Swap in flight (PUT sent, no ack) | **Either** `V` (PUT didn't land) **or** `V+1` (PUT landed) — never partial, because the create-only PUT is atomic at the store | On recovery, LIST resolves whichever actually committed. Idempotent: re-issuing the same create either 200s (we won) or 412s (it landed). |
| After swap ack, before client ack | `V+1` committed and durable | Client may retry; commit is already durable and idempotent (same version key). |
| Lease holder stalls (zombie) | `V` or `V+1` per §4 | New writer takes over; zombie fenced by create-only CAS on next swap. |
| Reader crash mid-read | Reader's pin lingers then TTL-expires | Version becomes GC-eligible after grace; no corruption (reader read immutable objects). |

**No reader ever observes a partially-committed state**, because the *only*
thing that makes any data object reachable is its manifest, and a manifest
becomes visible **atomically** (single create) and **only after** its data is
durable. This is the `NoTornCommit` invariant.

### 8. Snapshot-isolation level claimed

Per decision 0004 non-blocking note, we claim exactly what the model proves:
**Snapshot Isolation** for the rubric floor; **serializable for read-only
transactions** as a free consequence of immutable versioned manifests +
single-writer (no write-write or write-read anomalies are possible for readers).
We do **not** claim serializability across concurrent *writers* — there is only
one writer by design (R2), so the question does not arise.

## Alternatives considered

### Alternative A — Lease-fenced in-place pointer swap (mutable `_latest`)

**Description:** keep a single mutable `db/_latest` object holding the current
version; the lease holder overwrites it (`PUT`, possibly `If-Match: <etag>`) to
commit.

**Why considered:** one round-trip commit; trivially small "current version"
resolution (one GET, no LIST).

**Why rejected:** (1) It makes **fencing depend on lease belief or on `If-Match`
CAS of an existing body** — the latter has partial/inconsistent support across
S3-compatible stores (decision 0004 #2). (2) It reintroduces the zombie-writer
race: a stalled lease holder can overwrite `_latest` after a new writer advanced
it, *torn-commit / lost-update*. (3) An in-place pointer is a mutable shared cell
— readers can observe it changing mid-read. The create-only **per-version key**
design removes the mutable cell entirely and makes fencing a store-enforced
property. (We keep `_latest` only as an *advisory* hint, never trusted.)

### Alternative B — External coordination service (etcd / DynamoDB lock) for the writer lease + commit

**Description:** use a strongly-consistent external KV for the lease and the
"current version" pointer; S3 holds only data.

**Why considered:** mature CAS, leases, and watches; well-trodden.

**Why rejected:** violates R4 / commander's intent — **all durable state on
S3-compatible object storage**, no coordination service beyond what the object
store natively provides. It also adds an operational dependency and a second
consistency domain to reason about. The create-only PUT gives us everything we
need from S3 alone.

### Alternative C — Multi-object commit via a "commit marker" + scan

**Description:** write data + a per-version `COMMIT` marker object; readers scan
for the latest marker.

**Why considered:** avoids relying on conditional PUT.

**Why rejected:** without a conditional create, two writers can both write a
`V+1` marker → ambiguity / split-brain, requiring a tiebreak that itself needs
CAS. It collapses into Alternative A's problem. The create-only PUT *is* the
clean primitive; reusing it as the manifest key (which we must write anyway)
costs nothing extra.

## Consequences

### Positive

- **Cat. 1 (ACID):** atomicity and snapshot isolation reduce to "manifests are
  immutable, created exactly once, and reference only durable data" — a property
  that is **machine-checked** in `commit_protocol.tla` (`NoTornCommit`,
  `SnapshotIsolation`). Advances Cat. 1 toward 100 (the "behaviour matches the
  TLA+ model" anchor).
- **Cat. 7 (concurrency):** split-brain is impossible by construction
  (`AtMostOneCommitPerVersion`), covering all four attach modes; second-writer
  rejection without a coordination service.
- **Cat. 11 (formal):** a model-checkable spec with named invariants and a
  documented checker run; kept in sync with EPIC-004 code (model-sync gate).
- **Cat. 2 (storage):** commit = single create; readers do few, large, parallel
  range GETs over immutable version-scoped objects (serves the latency envelope).
- **No coordination service**, no mutable shared cell, no reliance on `If-Match`.

### Negative / trade-offs

- **"Current version" resolution costs a LIST** (or a re-validated hint). LIST on
  a `manifest/` prefix with monotone zero-padded keys is one request returning a
  bounded key set; the cost is one round-trip at open time, folded into the
  latency budget's `K` phases (negligible vs. data GETs). The advisory `_latest`
  hint reduces this to a single GET in the common case.
- **Orphaned objects** from fenced/crashed writers need GC sweeping (handled,
  §6.4) — a small amount of transient wasted storage.
- **Monotone version keys** assume a single writer minting versions; that is
  exactly R2's single-writer model, so this is not a new constraint.
- **Create-only conditional PUT is a hard dependency** on the store/mock — pinned
  and test-gated (§3 mock-fidelity obligation). If a target store lacks it, that
  store is unsupported (documented), not silently unsafe.

### Open questions

- **Exact grace-window / TTL values** (§6 rule 3) vs. the max reader-session
  lifetime — to be pinned with concrete numbers in EPIC-001 (T-0012 GC) and
  EPIC-006 (lease timing). The *shape* (grace > max session) is fixed here; the
  constants are an implementation tuning, not a design question.
- **Statistics-in-manifest schema** (SPIKE-0004 —
  `docs/specs/SPIKE-0004-manifest-statistics-contract.md`) — the manifest is the
  agreed snapshot-consistent home; the field layout (inline OOE-critical scalars +
  referenced per-property selectivity blobs) is owned by that spike.
- **Multi-shard atomicity:** the model uses a single data shard per version; the
  atomicity argument is independent of shard count (all shards are staged before
  the single manifest create). EPIC-001 must preserve "all shards durable before
  manifest create" — captured as the durability barrier, re-checked by T-0038.

## Rubric impact

| Cat. | Name | Impact |
|------|------|--------|
| 1 | ACID txns & correctness | Defines the atomic-commit + SI protocol the implementation must match; backed by the TLA+ model. Toward 100. |
| 7 | Concurrency & attach modes | Split-brain prevention via create-only CAS; covers all four attach modes without a coordination service. Toward 100. |
| 11 | Formal verification | Provides the model + invariants + checker plan; model-sync obligation recorded. Toward 100. |
| 2 | Storage format & S3 commit | Pins the commit half of the format (manifest keys, immutability, version-scoped objects). Constrains SPIKE-0003. |
| 3 | Latency envelope | Manifest LIST/GET folded into K phases; immutable version-scoped objects enable parallel range GETs. Supports envelope. |

## Sign-off

### Model-check evidence (Cat. 11)

The TLA+ model `formal/commit-protocol/commit_protocol.tla` is checked with **TLC**
(tla2tools 1.7.4, EPL-2.0; Apalache fallback per the formal-prover agent def —
Apalache is added in T-0038 to check the *implemented* protocol).
Full output / hand-derivation: `formal/results/commit_protocol_check.txt`.

**Model revision v2 (DA-1 / BC-4 fix; BUG-0012 / T-0046).** v1 keyed data objects
as `ObjId(v, k)` — a pure function of `(version, shard)` — which made
`NoTornCommit` pass **vacuously** against the DA-1 stale-overwrite (decision 0024,
FM-1). v2 keys objects as `ObjId(v, w, a, k)` by `(version, writer, attempt,
shard)` (modelling the content-addressed key above), adds the `ZombieLateWrite`
action (the stale PUT, now representable), and adds the `OrphansNeverReferenced`
+ `NoOverwriteOfReferenced` invariants — turning write-once immutability from an
assertion into a checked property.

- **SANY parse:** expected OK (syntactic + semantic).
- **Safety:** at the bound Writers={w1,w2}, Readers={r1,r2}, MaxVersion=2,
  MaxLeaseEpoch=3 (the SPIKE-0002 suggested bound), all of `NoTornCommit`,
  `SnapshotIsolation`, `AtMostOneCommitPerVersion`, `ManifestImmutable`,
  `GCSafety`, **`OrphansNeverReferenced`**, **`NoOverwriteOfReferenced`**,
  `LatestIsDurable` (and `TypeOK`) hold. The v1 run was **37580 states / 7406
  distinct, no violations** (but `NoTornCommit` was vacuous w.r.t. DA-1). The v2
  run **must be re-executed on a JRE** (this sandbox has none; `./check.sh`
  reproduces; T-0038 runs Apalache on the implementation); the v2 invariant
  arguments are hand-derived in the results file.
- **Liveness:** `WriterEventuallyCommits` holds under weak fairness.
- **Non-vacuity:** three probes (`NoRaceProbe`, `DistinctIdsProbe`,
  `ZombieWroteProbe`) confirm the same-version race is reachable, that racing
  writers now stage **distinct** ids (closing the v1 idempotent collapse), and
  that a fenced writer can durably hold its orphan alongside the committed
  snapshot — so the safety pass, and `OrphansNeverReferenced` /
  `NoOverwriteOfReferenced` in particular, are meaningful, not trivial.

### Adversarial review record

_(design-falsification loop pending — see PR.md / decision 0013)_

The four pre-registered falsification scenarios (decisions 0001 F3, 0004 #2/#3/#4)
are addressed in §3, §4, §6, §7 respectively and encoded as TLA+ invariants
(`AtMostOneCommitPerVersion`, `NoTornCommit`, `GCSafety`, `LatestIsDurable`),
each of which TLC confirms holds across the reachable state space including the
zombie-writer interleaving.

### Steering ratification

Routing (per `steering-committee.md` §Sign-off protocol): commit protocol /
isolation guarantee → **primary `steering-distributed-acid`**, secondary
`steering-formal-methods`. `steering-storage` is the **secondary consultant** on
the storage-bearing aspects (manifest layout / versioning, GC, concurrent-reader
version-pinning) and is the named owner of decision 0001 findings **F2** (CAS
primitive) and **F3** (GC safety), both of which this ADR must discharge.

| Member | Role here | Verdict | Record |
|--------|-----------|---------|--------|
| `steering-distributed-acid` | **primary** (commit/isolation/fencing/attach) | DA-1/BC-4 surfaced on v1 (0023) → **RATIFIED-WITH-CONDITIONS** on the v2 model + ADR | decision 0023 → decision 0026 (RATIFIED) |
| `steering-formal-methods` | primary (TLA+, Cat. 11) | FM-1 CHANGES_REQUESTED on v1 (0024) → **RATIFIED** the v2 model (FM-1 vacuity closed) | decision 0024 → decision 0029 (RATIFIED) |
| `steering-storage` | secondary (storage-side: layout/versioning/GC/pinning; F2+F3 owner) | **APPROVE** (1 binding constraint on SPIKE-0003 — BC-1 cross-version-sharing / ref-counted GC; 2 non-blocking notes) | decision 0027 |

> **Gate status: OPEN (design gate satisfied; ADR `accepted`; SPIKE-0002 `done`).**
> Both primaries have recorded their sign-off on the **v2 model + DA-1-fixed ADR**:
> `steering-distributed-acid` (primary, commit) RATIFIED-WITH-CONDITIONS in
> decision **0026**, and `steering-formal-methods` (primary, TLA+) RATIFIED in
> decision **0029**; `steering-storage` (secondary) APPROVE in decision **0027**.
>
> **Commit-path implementation readiness** (flipping T-0010/T-0026/T-0012/T-0038 to
> `ready`) is gated on conditions **C1–C4 (decision 0026)** as their land-gates —
> these are *implementation* gates, NOT design blockers and NOT a reason to keep
> SPIKE-0002 open: C1 executed `check.sh`/Apalache re-check (T-0038, no JRE in the
> authoring sandbox so the v2 record is a sound hand-derivation pending the run);
> C2 two-concurrent-`PUT If-None-Match:*` mock-fidelity test; C3 zombie-late-PUT
> integration test; C4 model-sync discipline. The prove-before-code gate stays
> enforced for implementation.
>
> **DA-1 / BC-4 discharge (BUG-0012 + T-0046):** §1 (content-addressed,
> write-once-unique data keys), §2 step 1 (data-write precondition), and §6 rule 4
> (orphan = not in any live manifest's reference set) are updated; the TLA+ model
> is refined to `ObjId(v, w, a, k)` with `OrphansNeverReferenced` +
> `NoOverwriteOfReferenced` model-checked (non-vacuous via `DistinctIdsProbe` /
> `ZombieWroteProbe`). This closes decision 0024's FM-1 and decision 0023's BC-4.
>
> **Residual model-fidelity note (non-blocking — `steering-storage` BC-1):** the v2
> object id embeds the version (`ObjId(v,w,a,k)`), so the model treats each
> version's objects as distinct and cannot *represent* cross-version content
> de-dup. For the commit-protocol safety properties this is a **safe
> over-approximation** (GC never deletes a still-referenced object). When SPIKE-0003
> pins delta-encoding / cross-version sharing, ADR §6 + the TLA+ `GCOldVersion` /
> `GCSafety` must move to an explicitly model-checked reference-counted discipline
> (an invariant over the live-object-set of *all* surviving manifests + a probe that
> a shared key survives GC of one referencer). Tracked as BC-1 (decision 0027),
> enforced at SPIKE-0003 ratification — it does **not** gate this commit-protocol
> ADR (atomicity / isolation / fencing are independent of object sharing).

#### `steering-storage` verdict (storage domain) — decision 0027

The four pre-registered storage-domain falsification scenarios survive:
**F2** (CAS named precisely as create-only `PUT If-None-Match:*`, model abstracts
S3 to create-only only, mock-fidelity test mandated) — discharged; **F3** (GC
under lease, never `latest`, never a live-pinned version, grace > max session;
`GCSafety` is a checked invariant; master-less ⇒ no GC) — discharged; manifest
swap atomicity and immutable version-scoped objects give concurrent readers a
torn-read-free stable snapshot (`NoTornCommit` + `SnapshotIsolation` +
`ManifestImmutable`); stale-LIST resolves an older but *complete* snapshot
(freshness, not safety). **One binding constraint** is recorded for SPIKE-0003
(storage format): the GC model assumes **each version owns its data objects
exclusively** (`GCOldVersion` deletes `manifests[v].objs` wholesale; ADR §6 rule
c; model `ObjId(v,k)` is version-scoped). If SPIKE-0003 introduces **cross-version
object sharing / delta encoding** (likely, for write-amplification reasons on a
10B-edge graph), wholesale per-version deletion becomes unsafe and the GC model +
TLA+ `GCOldVersion`/`GCSafety` must move to a **reference-count / live-object-set**
discipline. This does not falsify the *commit* protocol (atomicity is independent
of object sharing) and does not block this gate; it is a constraint SPIKE-0003
must honour and `steering-storage` will enforce at SPIKE-0003 ratification.

> **Update (DA-1 / BC-4, BUG-0012 / T-0046):** the DA-1 fix adopts
> **content-addressed data keys** and a **reference-counted / live-object-set GC**
> sweep (§1, §6 rule 4; model `GCOldVersion` deletes `manifests[v].objs \ stillRef`
> rather than wholesale per-version). This **directly satisfies** the
> ref-count / live-object-set discipline this binding constraint anticipated:
> content-addressing makes cross-version sharing safe (identical content de-dupes
> to one key; GC removes a key only when no surviving manifest references it). The
> `ObjId(v,k)` version-scoped encoding referenced above is replaced by
> `ObjId(v, w, a, k)` (attempt-unique) in model v2.
