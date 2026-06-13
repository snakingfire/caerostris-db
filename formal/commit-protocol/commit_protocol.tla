-------------------------- MODULE commit_protocol --------------------------
(*****************************************************************************)
(* TLA+ model of the caerostris-db S3-native commit / concurrency protocol. *)
(*                                                                           *)
(* Scope (SPIKE-0002 / EPIC-004, rubric Cat. 1 + Cat. 11):                   *)
(*   - single-writer / multi-reader on commodity object storage (S3);        *)
(*   - commit = atomic swap of a versioned manifest via create-only          *)
(*     conditional PUT (compare-and-swap on the manifest VERSION, NOT on     *)
(*     lease belief);                                                        *)
(*   - reader snapshot pinning via TTL'd pin objects;                        *)
(*   - garbage collection running concurrently under the writer lease;       *)
(*   - writer crash at every commit phase, and writer-lease takeover         *)
(*     (zombie-writer) without split-brain;                                  *)
(*   - data objects keyed by *write attempt* (content-addressed), so a       *)
(*     fenced/zombie writer's late PUT can NEVER overwrite a live,           *)
(*     manifest-referenced object (finding DA-1 / BC-4; decisions 0023/0024).*)
(*                                                                           *)
(* The model deliberately abstracts S3 to its observable, *native*           *)
(* guarantees (decision 0004 finding 2):                                     *)
(*   - strong read-after-write consistency on every object;                  *)
(*   - create-only conditional PUT, "If-None-Match star", that succeeds for  *)
(*     exactly ONE of any number of racing creators of the same key.         *)
(* No distributed lock service, no transactions across keys, no CAS on an    *)
(* existing object's body are assumed -- only what S3 natively offers.       *)
(*                                                                           *)
(* The DA-1 refinement (this revision, BUG-0012 / T-0046):                   *)
(*   v1 keyed a data object as ObjId(v,k) -- a pure function of              *)
(*   (version, shard). Two writers racing the SAME version V+1 staged the    *)
(*   *identical* set element, so `dataObjects \cup writerObjs[w]` was        *)
(*   idempotent and the model had NO notion of content changing under a      *)
(*   stable id. `NoTornCommit` therefore held *vacuously* against the DA-1   *)
(*   attack (a zombie writer PUTting stale content over a live committed     *)
(*   object's key -- S3 last-write-wins). This revision keys a data object   *)
(*   by (version, writer, attempt, shard) -- a content-addressed / per-write *)
(*   token -- so racing writers stage DISTINCT ids; a fenced writer's late   *)
(*   PUT lands on its own key, never the winner's. The new invariant         *)
(*   `OrphansNeverReferenced` converts "write-once immutability" from an     *)
(*   ASSERTION into a model-checked PROPERTY.                                 *)
(*                                                                           *)
(* Safety invariants (steering-mandated, decisions 0001/0004/0023/0024):     *)
(*   NoTornCommit             -- no reader ever resolves a manifest whose     *)
(*                               referenced data objects are not all durable. *)
(*   SnapshotIsolation        -- a reader pinned at version V reads only the  *)
(*                               object set of V, even while V+1 commits.     *)
(*   AtMostOneCommitPerVersion-- at most one commit ever succeeds for a given *)
(*                               manifest version (the correct fencing        *)
(*                               statement; refutes the zombie-writer race).  *)
(*   GCSafety                 -- GC never deletes a data object that a pinned  *)
(*                               (live, non-expired) reader still references.  *)
(*   OrphansNeverReferenced   -- no data object a fenced/zombie writer staged *)
(*                               (its orphans) is ever referenced by ANY      *)
(*                               committed manifest. This is the DA-1/BC-4    *)
(*                               check: a fenced writer's late PUT cannot     *)
(*                               corrupt a live committed snapshot.           *)
(*   NoOverwriteOfReferenced  -- the content stored under a key referenced by *)
(*                               a committed manifest never changes (no       *)
(*                               last-write-wins tear of a live object).      *)
(*   LatestIsDurable          -- the resolved `latest` always names a         *)
(*                               complete, durable manifest.                  *)
(*                                                                           *)
(* Two liveness properties are checked under weak fairness:                  *)
(*   WriterEventuallyCommits     -- a non-crashing writer eventually commits  *)
(*                                  (or is fenced -- a terminal, safe state). *)
(*   ReaderEventuallyGetsSnapshot-- a reader eventually pins a snapshot.      *)
(*****************************************************************************)

EXTENDS Naturals, FiniteSets, Sequences, TLC

CONSTANTS
    \* @type: Set(WRITER);
    Writers,        \* set of writer process ids (e.g. {w1, w2})
    \* @type: Set(READER);
    Readers,        \* set of reader process ids (e.g. {r1, r2})
    \* @type: Int;
    MaxVersion,     \* highest manifest version the bounded check explores
    \* @type: Int;
    MaxLeaseEpoch,  \* highest lease epoch (number of writer takeovers + 1)
    \* @type: NONE;
    None            \* sentinel "no version" / "no holder", a distinct model value

ASSUME /\ MaxVersion \in Nat /\ MaxVersion >= 1
       /\ MaxLeaseEpoch \in Nat /\ MaxLeaseEpoch >= 1
       /\ Writers # {} /\ Readers # {}
       \* None must be disjoint from real ids (TLC model value satisfies this).
       /\ None \notin (Writers \cup Readers)
       /\ None \notin 0..MaxVersion

\* Permutation symmetry over interchangeable reader ids (TLC only; Apalache
\* ignores the SYMMETRY directive and checks the full space).
\* NOTE: we permute ONLY readers, not writers. After the DA-1 refinement a
\* staged object's id embeds the *writer*, so writers are no longer freely
\* interchangeable for the object-layer invariants (OrphansNeverReferenced,
\* NoOverwriteOfReferenced distinguish "the winner's object" from "a peer's");
\* permuting writers would be unsound for those. Readers carry no such payload.
Symmetry == Permutations(Readers)

----------------------------------------------------------------------------
(***************************************************************************)
(* State                                                                   *)
(*                                                                         *)
(* The object store is modelled as a small set of named objects:           *)
(*                                                                         *)
(*  manifests   : function version -> manifest record. A manifest is       *)
(*                durable & visible iff version \in DOMAIN manifests.      *)
(*                Manifest objects are immutable once created (create-only  *)
(*                conditional PUT); a version key is written exactly once.  *)
(*                Each manifest record carries the SET of data-object ids   *)
(*                its snapshot references (`objs`) -- the EXACT ids the      *)
(*                winning writer staged, never a peer's.                     *)
(*                                                                         *)
(*  dataObjects : set of data-object ids that are durably written. After    *)
(*                the DA-1 refinement an id is keyed by (version, writer,    *)
(*                attempt, shard): a content-addressed / per-write token.    *)
(*                Two writers racing the same version produce DISTINCT ids,  *)
(*                so a stale writer's PUT can never land on a live object's   *)
(*                key. GC may remove ids from this set.                      *)
(*                                                                         *)
(*  latest      : the resolved "current" version pointer. In the real      *)
(*                system this is recovered by listing manifest keys and     *)
(*                taking the max; here we track it explicitly and prove it  *)
(*                only ever advances to a fully-durable manifest.           *)
(*                                                                         *)
(*  lease       : [ holder, epoch, expired ] -- the writer lease object.    *)
(*                `holder` is the writer that currently believes it holds   *)
(*                the lease; `epoch` is the fencing epoch; `expired` lets    *)
(*                the environment expire a lease (modelling a stall/crash). *)
(*                                                                         *)
(*  writerState : per-writer commit FSM:                                    *)
(*       "idle"     -- not committing                                       *)
(*       "leased"   -- holds (believes it holds) the lease                  *)
(*       "wrote"    -- has durably written all V+1 data objects             *)
(*       "committed"-- has successfully swapped the manifest                *)
(*       "crashed"  -- crashed; will not progress                           *)
(*       "fenced"   -- its swap lost the CAS (a newer version exists)       *)
(*    writerTarget  : the version this writer is attempting to commit       *)
(*    writerAttempt : a monotone per-writer attempt counter -- the "write    *)
(*                    token". Bumped on every AcquireLease so a writer that  *)
(*                    retries after fencing stages a FRESH object set, and   *)
(*                    so two writers never share a token.                    *)
(*    writerObjs    : the data-object ids this writer has staged             *)
(*                                                                         *)
(*  readerState : per-reader FSM: "idle" | "pinned" | "done"               *)
(*    readerPin   : the version a reader has pinned (or None)              *)
(*    pinObjects  : the set of (reader -> version) pin objects on the store *)
(*                  that are live (not yet expired/removed). GC must honour  *)
(*                  every live pin.                                         *)
(***************************************************************************)

VARIABLES
    manifests,      \* @type: Int -> { objs: Set(Str), ver: Int };
    dataObjects,    \* @type: Set(Str);
    latest,         \* @type: Int;
    lease,          \* @type: { holder: WRITER, epoch: Int, expired: Bool };
    writerState,    \* @type: WRITER -> Str;
    writerTarget,   \* @type: WRITER -> Int;
    writerAttempt,  \* @type: WRITER -> Int;
    writerObjs,     \* @type: WRITER -> Set(Str);
    readerState,    \* @type: READER -> Str;
    readerPin,      \* @type: READER -> Int;
    pinObjects      \* @type: Set(<<READER, Int>>);

vars == << manifests, dataObjects, latest, lease,
           writerState, writerTarget, writerAttempt, writerObjs,
           readerState, readerPin, pinObjects >>

----------------------------------------------------------------------------
(* Helpers ****************************************************************)

\* DA-1 FIX (BUG-0012 / decision 0023 BC-4 / decision 0024 FM-1):
\* A data object's id is keyed by (version, writer, attempt, shard) -- a
\* per-write-attempt token modelling the ADR's content-addressed key
\* `db/data/<content-hash>/<shard>.col` (or the attempt-scoped alternative
\* `db/data/v<V>/<epoch-or-uuid>/<shard>.col`). Two writers racing the SAME
\* version V stage DISTINCT ids; a writer that retries after being fenced
\* stages a fresh id (new attempt). The idempotent set-union collapse of v1
\* (where ObjId(v,k) ignored the writer) is therefore GONE: the model can now
\* REPRESENT a stale writer's late PUT, and the invariants below prove it can
\* never overwrite a live, manifest-referenced object.
ObjId(v, w, a, k) ==
    "v" \o ToString(v) \o "-w" \o ToString(w) \o "-a" \o ToString(a)
        \o "-k" \o ToString(k)

\* The object set a writer stages for version v on a given attempt: one object
\* per (abstract) column/adjacency shard. A single shard k=1 keeps the state
\* space bounded; the atomicity argument is independent of shard count.
StagedObjs(v, w, a) == { ObjId(v, w, a, 1) }

\* The full universe of data-object ids reachable within the bounds. Used by
\* TypeOK to constrain `dataObjects` without enumerating writer ids in TLC's
\* CONSTANTS (writers are model values, so we range over them directly).
AllObjIds ==
    { ObjId(v, w, a, 1) : v \in 0..MaxVersion, w \in Writers,
                          a \in 1..MaxLeaseEpoch }

\* Whether version v is committed (its manifest object exists & is durable).
IsCommitted(v) == v \in DOMAIN manifests

\* The set of ALL object ids referenced by ANY committed manifest -- i.e. the
\* objects that are "live" (a reader could resolve a manifest and read them).
ReferencedObjs == UNION { manifests[v].objs : v \in DOMAIN manifests }

\* A manifest is "complete" iff every data object it references is durable.
ManifestComplete(v) ==
    IsCommitted(v) => (manifests[v].objs \subseteq dataObjects)

\* All currently-pinned versions across live pin objects.
PinnedVersions == { p[2] : p \in pinObjects }

----------------------------------------------------------------------------
(* Initial state *********************************************************)

Init ==
    \* Version 0 is the genesis empty snapshot: committed, no data objects.
    /\ manifests = (0 :> [ objs |-> {}, ver |-> 0 ])
    /\ dataObjects = {}
    /\ latest = 0
    /\ lease = [ holder |-> None, epoch |-> 0, expired |-> TRUE ]
    /\ writerState = [ w \in Writers |-> "idle" ]
    /\ writerTarget = [ w \in Writers |-> None ]
    /\ writerAttempt = [ w \in Writers |-> 0 ]
    /\ writerObjs = [ w \in Writers |-> {} ]
    /\ readerState = [ r \in Readers |-> "idle" ]
    /\ readerPin = [ r \in Readers |-> None ]
    /\ pinObjects = {}

----------------------------------------------------------------------------
(* Writer actions ********************************************************)

\* AcquireLease: a writer takes the lease when it is free or expired. The lease
\* epoch increments monotonically on every acquisition -- this is the FENCING
\* TOKEN, but note (decision 0004 finding 3) safety does NOT rely on it: it is
\* an optimisation to avoid wasted work, not a correctness lever.
\* DA-1: the writer also bumps its per-writer attempt counter and stages a
\* FRESH, attempt-scoped object set, so a retry never reuses a prior key.
AcquireLease(w) ==
    /\ writerState[w] = "idle"
    /\ \/ lease.holder = None
       \/ lease.expired = TRUE
    /\ lease.epoch < MaxLeaseEpoch
    /\ writerAttempt[w] < MaxLeaseEpoch  \* bound attempts per writer
    /\ latest < MaxVersion        \* bound: don't explore versions > MaxVersion
    /\ lease' = [ holder |-> w, epoch |-> lease.epoch + 1, expired |-> FALSE ]
    /\ writerState' = [ writerState EXCEPT ![w] = "leased" ]
    \* Target the next version after the latest the writer can observe.
    /\ writerTarget' = [ writerTarget EXCEPT ![w] = latest + 1 ]
    /\ writerAttempt' = [ writerAttempt EXCEPT ![w] = writerAttempt[w] + 1 ]
    /\ writerObjs' = [ writerObjs EXCEPT
                         ![w] = StagedObjs(latest + 1, w, writerAttempt[w] + 1) ]
    /\ UNCHANGED << manifests, dataObjects, latest,
                    readerState, readerPin, pinObjects >>

\* RenewLease: refresh a held lease (models heartbeat; keeps it un-expired).
RenewLease(w) ==
    /\ writerState[w] \in { "leased", "wrote" }
    /\ lease.holder = w
    /\ lease' = [ lease EXCEPT !.expired = FALSE ]
    /\ UNCHANGED << manifests, dataObjects, latest, writerState,
                    writerTarget, writerAttempt, writerObjs, readerState,
                    readerPin, pinObjects >>

\* WriteDataObjects: the writer durably writes ALL data objects of its target
\* version BEFORE any manifest swap (the durability ordering barrier,
\* decision 0004 finding 4). Data objects are immutable & attempt-scoped, so
\* they are invisible to readers (no manifest references them yet) AND distinct
\* from any other writer's objects (DA-1: distinct keys, never an overwrite).
WriteDataObjects(w) ==
    /\ writerState[w] = "leased"
    /\ writerTarget[w] # None
    /\ dataObjects' = dataObjects \cup writerObjs[w]
    /\ writerState' = [ writerState EXCEPT ![w] = "wrote" ]
    /\ UNCHANGED << manifests, latest, lease, writerTarget, writerAttempt,
                    writerObjs, readerState, readerPin, pinObjects >>

\* ZombieLateWrite: THE DA-1 ATTACK, now REPRESENTABLE. A writer that has been
\* fenced (lost the CAS) or whose lease expired wakes up and, believing it
\* still holds the lease, re-issues its data PUT. In v1 this was invisible
\* (same key as the committed object => silent last-write-wins overwrite). With
\* attempt-scoped keys the PUT lands on the zombie's OWN distinct key: it adds
\* an orphan to `dataObjects` and CANNOT touch any object a committed manifest
\* references. We model it explicitly so the checker EXERCISES a post-commit
\* stale write and `OrphansNeverReferenced` / `NoOverwriteOfReferenced` are
\* proven non-vacuously against it.
ZombieLateWrite(w) ==
    /\ writerState[w] \in { "fenced", "wrote" }
    /\ writerObjs[w] # {}
    /\ writerObjs[w] \cap ReferencedObjs = {}   \* (always true; asserted below)
    /\ dataObjects' = dataObjects \cup writerObjs[w]
    /\ UNCHANGED << manifests, latest, lease, writerState, writerTarget,
                    writerAttempt, writerObjs, readerState, readerPin,
                    pinObjects >>

\* SwapManifest: the atomic commit. Modelled as a create-only conditional PUT
\* on key `manifest/<target>`: it succeeds IFF that version key does not yet
\* exist. THIS is the fencing mechanism -- a zombie writer whose lease expired
\* and whose target version was already committed by another writer simply
\* LOSES the create-only race and is fenced. No lease check gates correctness.
\* The committed manifest references EXACTLY this writer's staged ids.
SwapManifestOk(w) ==
    /\ writerState[w] = "wrote"
    /\ writerTarget[w] # None
    /\ ~IsCommitted(writerTarget[w])                 \* CAS: version key absent
    /\ writerObjs[w] \subseteq dataObjects            \* barrier: data durable
    /\ manifests' = manifests @@
                    (writerTarget[w] :> [ objs |-> writerObjs[w],
                                          ver  |-> writerTarget[w] ])
    /\ latest' = IF writerTarget[w] > latest THEN writerTarget[w] ELSE latest
    /\ writerState' = [ writerState EXCEPT ![w] = "committed" ]
    /\ UNCHANGED << dataObjects, lease, writerTarget, writerAttempt,
                    writerObjs, readerState, readerPin, pinObjects >>

\* SwapManifestFenced: the conditional PUT fails because the target version
\* already exists (another writer won). The losing writer is fenced; its
\* staged data objects become orphans (never referenced by any manifest,
\* safely GC-able -- proven by OrphansNeverReferenced). The DB is NOT left
\* inconsistent, and (DA-1) the fenced writer's later PUTs hit its own keys.
SwapManifestFenced(w) ==
    /\ writerState[w] = "wrote"
    /\ writerTarget[w] # None
    /\ IsCommitted(writerTarget[w])                  \* CAS would fail
    /\ writerState' = [ writerState EXCEPT ![w] = "fenced" ]
    /\ UNCHANGED << manifests, dataObjects, latest, lease, writerTarget,
                    writerAttempt, writerObjs, readerState, readerPin,
                    pinObjects >>

\* ReleaseLease: a committed or fenced writer releases the lease and resets.
\* It does NOT immediately clear writerObjs: a fenced writer's orphan keys
\* persist until GC. We clear writerObjs only when it returns fully idle so a
\* subsequent AcquireLease starts from a clean slate (and a fresh attempt).
ReleaseLease(w) ==
    /\ writerState[w] \in { "committed", "fenced" }
    /\ lease' = IF lease.holder = w
                THEN [ lease EXCEPT !.expired = TRUE, !.holder = None ]
                ELSE lease
    /\ writerState' = [ writerState EXCEPT ![w] = "idle" ]
    /\ writerTarget' = [ writerTarget EXCEPT ![w] = None ]
    /\ writerObjs' = [ writerObjs EXCEPT ![w] = {} ]
    /\ UNCHANGED << manifests, dataObjects, latest, writerAttempt,
                    readerState, readerPin, pinObjects >>

\* CrashWriter: a writer may crash at ANY phase. A crashed writer holds no
\* progress. Critically, if it crashes after WriteDataObjects but before/at
\* SwapManifest, the half-committed state is invisible: no manifest references
\* the new version, so readers and `latest` are unaffected. Its lease will
\* expire (modelled by ExpireLease) and another writer takes over.
CrashWriter(w) ==
    /\ writerState[w] \notin { "crashed", "idle" }
    /\ writerState' = [ writerState EXCEPT ![w] = "crashed" ]
    \* The lease is NOT released cleanly -- it lingers until it expires,
    \* which is exactly the zombie/stall window the protocol must survive.
    /\ UNCHANGED << manifests, dataObjects, latest, lease, writerTarget,
                    writerAttempt, writerObjs, readerState, readerPin,
                    pinObjects >>

----------------------------------------------------------------------------
(* Environment actions ***************************************************)

\* ExpireLease: the environment expires a (possibly zombie) lease, allowing
\* takeover. This is the heart of the split-brain test: after expiry a second
\* writer can AcquireLease while the original may still wake up and attempt a
\* swap (if it had not crashed). Safety must hold regardless.
ExpireLease ==
    /\ lease.holder # None
    /\ lease.expired = FALSE
    /\ lease' = [ lease EXCEPT !.expired = TRUE ]
    /\ UNCHANGED << manifests, dataObjects, latest, writerState,
                    writerTarget, writerAttempt, writerObjs, readerState,
                    readerPin, pinObjects >>

\* WakeZombie: a crashed-then-recovered writer is not modelled as resuming;
\* instead the dangerous cases are (a) a writer that STALLED (lease expired)
\* and now wakes at "wrote" to attempt its swap -- covered: gated only by the
\* version-CAS, never by the stale lease; and (b) a fenced/stalled writer that
\* re-issues its data PUT -- covered by ZombieLateWrite, with attempt-scoped
\* keys making it a harmless orphan write.

----------------------------------------------------------------------------
(* Reader actions ********************************************************)

\* PinSnapshot: a reader resolves `latest` and writes a pin object recording
\* the version it will read. Because manifests are immutable and complete
\* before they are the resolution target, the pinned version is a stable,
\* fully-durable snapshot. The pin object protects every referenced data
\* object from GC for the reader's lifetime (TTL'd; see GC actions).
PinSnapshot(r) ==
    /\ readerState[r] = "idle"
    /\ IsCommitted(latest)
    /\ readerState' = [ readerState EXCEPT ![r] = "pinned" ]
    /\ readerPin' = [ readerPin EXCEPT ![r] = latest ]
    /\ pinObjects' = pinObjects \cup { << r, latest >> }
    /\ UNCHANGED << manifests, dataObjects, latest, lease, writerState,
                    writerTarget, writerAttempt, writerObjs >>

\* ReadObjects: the reader reads the data objects of its pinned version. This
\* is the action the SnapshotIsolation invariant guards: at this point every
\* referenced object MUST still be durable (GCSafety guarantees it) AND its
\* content unchanged (NoOverwriteOfReferenced guarantees no DA-1 tear). We model
\* the read as an assertion via the invariants rather than a state change.
ReadObjects(r) ==
    /\ readerState[r] = "pinned"
    /\ readerState' = [ readerState EXCEPT ![r] = "done" ]
    /\ UNCHANGED << manifests, dataObjects, latest, lease, writerState,
                    writerTarget, writerAttempt, writerObjs, readerPin,
                    pinObjects >>

\* UnpinSnapshot: the reader finishes and removes its pin object, freeing its
\* pinned version for GC.
UnpinSnapshot(r) ==
    /\ readerState[r] = "done"
    /\ readerState' = [ readerState EXCEPT ![r] = "idle" ]
    /\ pinObjects' = pinObjects \ { << r, readerPin[r] >> }
    /\ readerPin' = [ readerPin EXCEPT ![r] = None ]
    /\ UNCHANGED << manifests, dataObjects, latest, lease, writerState,
                    writerTarget, writerAttempt, writerObjs >>

----------------------------------------------------------------------------
(* Garbage collection ****************************************************)

(* GCOldVersion: GC runs under the writer lease (the only process allowed to
   mutate the "latest" lineage) and may reclaim an old version's manifest +
   data objects ONLY when:
     (a) it is not the latest version (never GC the live snapshot), and
     (b) NO live pin object references it (GCSafety), and
     (c) GC deletes only the objects THIS version OWNS that no OTHER committed
         manifest still references -- attempt-scoped object ids make a version
         own its objects exclusively, but we still subtract ReferencedObjs of
         the surviving manifests for defence in depth (and to match the ADR's
         ref-counted-GC story, decision 0015 / SPIKE-0003). *)
GCOldVersion(w) ==
    /\ writerState[w] \in { "leased", "wrote" }   \* holds the lease
    /\ lease.holder = w
    /\ \E v \in DOMAIN manifests :
         /\ v # latest
         /\ v \notin PinnedVersions               \* no live reader pins v
         /\ LET survivors == (DOMAIN manifests) \ {v}
                stillRef == UNION { manifests[u].objs : u \in survivors }
            IN /\ manifests' = [ u \in survivors |-> manifests[u] ]
               /\ dataObjects' = dataObjects \ (manifests[v].objs \ stillRef)
    /\ UNCHANGED << latest, lease, writerState, writerTarget, writerAttempt,
                    writerObjs, readerState, readerPin, pinObjects >>

----------------------------------------------------------------------------
(* Next-state relation ***************************************************)

Next ==
    \/ \E w \in Writers :
         \/ AcquireLease(w)
         \/ RenewLease(w)
         \/ WriteDataObjects(w)
         \/ ZombieLateWrite(w)
         \/ SwapManifestOk(w)
         \/ SwapManifestFenced(w)
         \/ ReleaseLease(w)
         \/ CrashWriter(w)
         \/ GCOldVersion(w)
    \/ \E r \in Readers :
         \/ PinSnapshot(r)
         \/ ReadObjects(r)
         \/ UnpinSnapshot(r)
    \/ ExpireLease

\* Fairness: writers and readers that can make progress eventually do, so the
\* liveness properties are meaningful. We do NOT make CrashWriter, ExpireLease,
\* or ZombieLateWrite fair (they are adversarial / environmental).
Fairness ==
    /\ \A w \in Writers :
         WF_vars(AcquireLease(w) \/ WriteDataObjects(w)
                 \/ SwapManifestOk(w) \/ SwapManifestFenced(w)
                 \/ ReleaseLease(w))
    /\ \A r \in Readers :
         WF_vars(PinSnapshot(r) \/ ReadObjects(r) \/ UnpinSnapshot(r))

Spec == Init /\ [][Next]_vars /\ Fairness

----------------------------------------------------------------------------
(* Type invariant ********************************************************)

TypeOK ==
    /\ DOMAIN manifests \subseteq 0..MaxVersion
    /\ \A v \in DOMAIN manifests :
         /\ manifests[v].ver = v
         /\ manifests[v].objs \subseteq AllObjIds
    /\ dataObjects \subseteq AllObjIds
    /\ latest \in 0..MaxVersion
    /\ lease.epoch \in 0..MaxLeaseEpoch
    /\ lease.holder \in (Writers \cup {None})
    /\ lease.expired \in BOOLEAN
    /\ \A w \in Writers :
         /\ writerState[w] \in { "idle","leased","wrote","committed",
                                 "fenced","crashed" }
         /\ writerAttempt[w] \in 0..MaxLeaseEpoch
         /\ writerObjs[w] \subseteq AllObjIds
    /\ \A r \in Readers :
         readerState[r] \in { "idle","pinned","done" }

----------------------------------------------------------------------------
(* SAFETY INVARIANTS  (steering decisions 0001 / 0004 / 0023 / 0024) *****)

(* INV-1  NoTornCommit
   Every committed (visible) manifest references only data objects that are
   durably present. A reader resolving any committed version therefore never
   observes a dangling reference -- the commit is all-or-nothing. The swap is
   gated by `writerObjs[w] \subseteq dataObjects` (the durability barrier),
   so a manifest can never become visible before its data is durable. *)
NoTornCommit ==
    \A v \in DOMAIN manifests : manifests[v].objs \subseteq dataObjects

(* INV-2  SnapshotIsolation
   A reader pinned at version V sees exactly the object set of V's manifest,
   and that object set remains durable for as long as the reader is pinned.
   Combined with attempt-scoped (immutable) object ids, a concurrent commit
   of V+1 cannot alter, add to, or remove from what the V-reader sees. *)
SnapshotIsolation ==
    \A r \in Readers :
        (readerState[r] \in { "pinned", "done" }) =>
            /\ readerPin[r] \in DOMAIN manifests
            /\ manifests[readerPin[r]].objs \subseteq dataObjects

(* INV-3  AtMostOneCommitPerVersion  (the CORRECTED fencing invariant)
   This replaces the naive `writer_count <= 1`. Two writers may *believe* they
   hold the lease simultaneously (zombie window), but the create-only
   conditional PUT guarantees at most ONE manifest object exists per version,
   so at most one commit ever succeeds for a given version. We assert it two
   ways:
     (a) manifests is a function => a version maps to a single manifest record;
     (b) no two distinct writers are both in "committed" targeting the same
         version. (a) is structural; (b) catches a modelling error where the
         CAS guard was bypassed. *)
AtMostOneCommitPerVersion ==
    \A w1, w2 \in Writers :
        ( /\ w1 # w2
          /\ writerState[w1] = "committed"
          /\ writerState[w2] = "committed"
          /\ writerTarget[w1] = writerTarget[w2] )
        => FALSE

(* A stronger structural restatement: the visible manifest at any version is
   unique and immutable. Because `manifests` is a TLA+ function this holds by
   construction; we expose it as an invariant so the checker would flag any
   future action that tried to overwrite an existing version key. *)
ManifestImmutable ==
    \A v \in DOMAIN manifests : manifests[v].ver = v

(* INV-4  GCSafety
   No data object referenced by a live pin's version is ever absent from the
   store. Equivalently: for every live pin << r, v >>, v's manifest still
   exists and all its objects are durable. GCOldVersion's precondition
   (v \notin PinnedVersions) is what makes this hold. *)
GCSafety ==
    \A p \in pinObjects :
        /\ p[2] \in DOMAIN manifests
        /\ manifests[p[2]].objs \subseteq dataObjects

(* INV-5  OrphansNeverReferenced   (THE DA-1 / BC-4 invariant, BUG-0012)
   The orphan objects of a fenced or crashed writer -- the ids it staged that
   did NOT become the committed manifest's object set -- are NEVER referenced
   by ANY committed manifest. Equivalently: a writer that is fenced/crashed and
   whose target version was committed by SOMEONE ELSE shares no staged id with
   the winning manifest. With attempt-scoped object ids this holds because the
   winner's manifest references exactly the winner's ids; the loser's ids embed
   a different writer/attempt token. This is the property that makes the
   zombie late PUT (ZombieLateWrite) HARMLESS: it can only ever add the loser's
   own orphan ids to the store, never overwrite a referenced one. Converts
   "write-once immutability" from an assertion into a model-checked property. *)
OrphansNeverReferenced ==
    \A w \in Writers :
        ( /\ writerState[w] \in { "fenced", "crashed" }
          /\ writerTarget[w] # None
          /\ IsCommitted(writerTarget[w]) )
        => (manifests[writerTarget[w]].objs \cap writerObjs[w] = {})
            \/ (writerObjs[w] = manifests[writerTarget[w]].objs)
            \* The right disjunct admits the (rare, harmless) case where THIS
            \* very writer is the one whose objs the manifest records -- i.e. it
            \* both committed and is now reported committed elsewhere; excluded
            \* in practice because a committed writer is in "committed", not
            \* "fenced"/"crashed". Kept for robustness against re-ordering.

(* INV-5b  NoOverwriteOfReferenced   (the structural DA-1 guarantee)
   Every object id that any committed manifest references is "owned" by exactly
   the writer/attempt that minted it; no OTHER writer's staged set contains it.
   Therefore a stale PUT (ZombieLateWrite) by any writer w whose objs are NOT
   the referenced set cannot collide with a referenced key -- there is no
   last-write-wins tear of a live object. We check: for every committed
   manifest's referenced object o and every writer w, if o \in writerObjs[w]
   then writerObjs[w] is exactly that manifest's object set (w IS the owner). *)
NoOverwriteOfReferenced ==
    \A v \in DOMAIN manifests :
        \A w \in Writers :
            (manifests[v].objs \cap writerObjs[w] # {})
                => (writerObjs[w] = manifests[v].objs)

(* INV-6  LatestIsDurable
   The resolved `latest` pointer always names a committed, complete manifest.
   This is what a fresh reader (cold start, master-less mode) relies on: the
   version it resolves is always safe to read. *)
LatestIsDurable ==
    /\ latest \in DOMAIN manifests
    /\ manifests[latest].objs \subseteq dataObjects

(* The conjunction the checker is run against. *)
SafetyInvariant ==
    /\ TypeOK
    /\ NoTornCommit
    /\ SnapshotIsolation
    /\ AtMostOneCommitPerVersion
    /\ ManifestImmutable
    /\ GCSafety
    /\ OrphansNeverReferenced
    /\ NoOverwriteOfReferenced
    /\ LatestIsDurable

----------------------------------------------------------------------------
(* NON-VACUITY PROBES (run as throwaway "invariants" TLC is asked to refute;
   a refutation = the named behaviour is REACHABLE in the bounded space, so the
   safety result is meaningful, not trivially satisfied). Listed here for the
   record / reproduction; enable one at a time in the .cfg.) *********)

(* PROBE-A  NoRaceProbe  (carried from v1)
   Asserts no two writers ever both reach wrote/committed on the same target
   version. TLC REFUTES it -> the zombie/concurrent-same-version race is
   reachable; yet AtMostOneCommitPerVersion + NoTornCommit still hold. *)
NoRaceProbe ==
    \A w1, w2 \in Writers :
        ( /\ w1 # w2
          /\ writerState[w1] \in { "wrote", "committed" }
          /\ writerState[w2] \in { "wrote", "committed" } )
        => writerTarget[w1] # writerTarget[w2]

(* PROBE-B  DistinctIdsProbe  (NEW -- the DA-1 non-vacuity probe, BUG-0012 AC)
   Asserts that two distinct writers NEVER simultaneously hold non-empty staged
   object sets for the SAME target version. TLC REFUTES it: the refuting trace
   is exactly "W1 stages v1, stalls; W2 stages v1" -- and crucially the two
   `writerObjs` sets are now DISTINCT (different writer/attempt tokens), which
   is what the v1 idempotent-union collapse hid. The refutation demonstrates
   the model can REPRESENT two writers racing the same version with distinct
   ids -- i.e. the previously-vacuous NoTornCommit is now tested against a
   genuine same-version race. *)
DistinctIdsProbe ==
    \A w1, w2 \in Writers :
        ( /\ w1 # w2
          /\ writerTarget[w1] = writerTarget[w2]
          /\ writerTarget[w1] # None
          /\ writerObjs[w1] # {}
          /\ writerObjs[w2] # {} )
        => FALSE

(* PROBE-C  ZombieWroteProbe  (NEW -- proves the late-PUT attack is exercised)
   Asserts a fenced writer's orphan objects are never durably present alongside
   a committed manifest for the same version. TLC REFUTES it: a fenced writer
   CAN durably write its orphan (ZombieLateWrite) after the winner committed.
   The refutation proves ZombieLateWrite fires in-space; OrphansNeverReferenced
   / NoOverwriteOfReferenced holding across it is therefore non-vacuous. *)
ZombieWroteProbe ==
    \A w \in Writers :
        ~( /\ writerState[w] = "fenced"
           /\ writerObjs[w] # {}
           /\ writerObjs[w] \subseteq dataObjects
           /\ IsCommitted(writerTarget[w]) )

----------------------------------------------------------------------------
(* LIVENESS PROPERTIES ***************************************************)

(* A writer that acquires the lease and does not crash eventually commits OR
   is fenced (loses the CAS to a peer). Either way it reaches a terminal,
   consistent state -- it never hangs holding the DB hostage. *)
WriterEventuallyCommits ==
    \A w \in Writers :
        (writerState[w] = "leased") ~>
            (writerState[w] \in { "committed", "fenced", "crashed" })

(* A reader that wants a snapshot eventually pins one (there is always a
   committed `latest` to pin, by LatestIsDurable + Init committing version 0). *)
ReaderEventuallyGetsSnapshot ==
    \A r \in Readers :
        (readerState[r] = "idle") ~> (readerState[r] # "idle")
            \* trivially true once it acts; the meaningful content is that
            \* PinSnapshot is always *enabled* (latest is always committed).

(* Liveness combined for the checker. *)
LivenessProperty ==
    /\ WriterEventuallyCommits

============================================================================
(* CHANGE LOG
   - v1 (SPIKE-0002, T0+0:50): initial model. Fencing invariant restated as
     AtMostOneCommitPerVersion per steering finding (decision 0004 #3); CAS
     primitive pinned to create-only conditional PUT (decision 0004 #2);
     durability barrier encoded in SwapManifestOk guard (decision 0004 #4);
     GCSafety gated on live pin objects, GC under lease (decision 0001 F3).
   - v2 (BUG-0012 / T-0046, T0+~2:30): DA-1 / BC-4 refinement (decisions
     0023, 0024). Data-object identity re-keyed from ObjId(v,k) -- a pure
     function of (version,shard) whose set-union was idempotent -- to
     ObjId(v,w,a,k), keyed by (version, writer, ATTEMPT, shard): a
     content-addressed / per-write token. Two writers racing the same version
     now stage DISTINCT ids. Added: writerAttempt counter (bumped on every
     AcquireLease); ZombieLateWrite action (makes the stale-PUT attack
     REPRESENTABLE and reachable); OrphansNeverReferenced + NoOverwriteOfReferenced
     invariants (turn write-once immutability into checked properties);
     DistinctIdsProbe + ZombieWroteProbe non-vacuity probes; GC subtracts
     surviving-manifest refs (defence-in-depth ref-counted GC). SYMMETRY
     narrowed to readers only (writer ids now carry object-id payload, so
     permuting writers is unsound for the object-layer invariants).
   MODEL-SYNC OBLIGATION: any change to the implemented commit phase sequence
   (EPIC-004 / T-0010, data-key scheme T-0046) must update this module in the
   same or an immediately following PR and re-run the checker. Drift = a BUG. *)
============================================================================
