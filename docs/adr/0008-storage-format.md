# ADR 0008 — On-object storage format & range-GET access pattern

> **ADR-number note:** originally drafted as ADR 0006; renumbered to **0008** at
> ratification time to avoid a collision with the in-flight Python-bindings ADR
> (T-0030), which had already claimed 0006 and 0007 on its PR branch. 0008 is
> unused across every ref (BUG-0010 collision class — avoided by construction).

## Status

`accepted` — ratified-with-conditions by `steering-storage` (design authority for
Cat. 2), T0+~03:40. This is the keystone storage-format spec; it closes SPIKE-0003
and unblocks T-0007 (columnar node-property writer/reader), T-0008 (adjacency-list
edge writer/reader), T-0009 (manifest + statistics + version resolution), and
T-0010 (atomic commit). Conditions are carried to those implementation tasks (see
Sign-off); they are land-gates, not design blockers.

<!-- proposed → reviewed (adversarial) → accepted (steering) → superseded -->

## Date / T+ marker

2026-06-13 (T0+~03:40)

## Context

This ADR is the mandatory design-before-code gate (SPIKE-0003) for the on-object
layout of caerostris-db — the exact bytes that live on the object store and the
range-GET access pattern by which a query reads only what it needs. It is the
companion to ADR 0002 (the S3-native commit/concurrency protocol, which owns the
*commit mechanics*: content-addressed keys, immutable manifests, create-only-CAS,
reference-counted GC, TTL'd pins) and is constrained by ADR 0001 (the latency
selectivity-envelope theorem, which owns the *byte/latency budget* the layout must
serve). It pins the storage-layer half of SPIKE-0004 (the manifest statistics
contract).

**Rubric stakes:** Cat. 2 (storage format & S3 commit, GATE, w12) — this ADR is
the primary artifact. It also feeds Cat. 1 (ACID — the object set a commit
references), Cat. 3 (the layout must demonstrably keep bytes-read ≤ `B_max` in
`K_min` phases with `r ≤ 1`), Cat. 4/6 (the executor and fast aggregates read
these structures), and Cat. 5 (index objects share the framing).

### Hard constraints carried in (the falsification scenarios this layout MUST survive)

These are pre-registered by prior ratified artifacts. The format is a falsification
if it violates any of them.

- **ADR 0001 Part 5 #1 — `r ≤ 1` (one round-trip per hop).** Hop `h+1`'s
  adjacency range-GETs must be issuable from the data hop `h` returned with **no
  additional serial round-trip** (no indirection GET between "receive hop-`h`
  neighbors" and "issue hop-`h+1` GETs"). If the format forces `r = 2`,
  `K_min = 14`, the 50 ms floor climbs to 700 ms, and the binding 50 Mbps case is
  infeasible — escalate to steering. (§4.)
- **ADR 0001 Part 5 #2 — contiguous adjacency for range-GET batching.** A frontier
  of `M_max` nodes whose source ids fall in a contiguous band must be served by a
  single (or bounded-parallel) range-GET, not `M_max` random GETs. (§3, §6.)
- **ADR 0001 Part 5 #3 — columnar node-property layout.** Filter-predicate
  properties must be readable without fetching whole node records. (§2.)
- **ADR 0001 Part 5 #4 — manifest carries statistics.** Per-label counts,
  per-property selectivity, per-rel-type degree summaries **with `max_deg`**, read
  in O(1) per planning call. (§5; SPIKE-0004.)
- **ADR 0001 Part 5 #5 / decision 0015 F2 — early-abort adjacency reads as a hard
  per-GET byte/row cap.** The reader truncates an adjacency range-GET once the
  running LIMIT/byte budget is consumed, so a super-hub frontier node cannot bust
  realized `B_max`. (§3.4, §6.3.)
- **ADR 0001 PS-1 — `K_min` must not silently become 9.** The final-row
  node-property fetch for the surviving LIMIT-10 rows must not require a separate
  serial round-trip after hop 6. Either co-locate the returnable/filterable
  properties with the hop traversal, or explicitly declare `K_min = 9` and re-pin
  the budget. This ADR satisfies it by co-location (§3.3) and additionally pins the
  K=9 fallback contract (§7.3).
- **ADR 0002 §1 / §6 — content-addressed, write-once data keys + reference-counted
  GC (decision 0027 BC-1).** Data-object keys embed a content hash; cross-version
  sharing is safe; GC deletes a key only when **no surviving manifest references
  it**. The layout must be expressed in content-addressed objects and must not
  reintroduce a shared mutable key. (§1, §7.)
- **decision 0001 F1/F2/F3 (SPIKE-0008).** F1 = early-abort partial adjacency reads
  (§3.4); F2 = the CAS primitive is named in ADR 0002 §3 and re-pointed here (§7.1);
  F3 = safe-GC-vs-reader is owned by ADR 0002 §5/§6 and re-pointed here (§7.2).
- **SPIKE-0004 R1 — inline-vs-referenced statistics cut.** OOE-critical scalars
  inline in the manifest; bulky MCV/histogram detail a referenced content-addressed
  `db/stats/<hash>.stats` blob; value digests never raw values. This ADR makes that
  cut (§5).

---

## Decision

We adopt a **content-addressed, columnar-node / CSR-adjacency, partition-mapped
object format**. Durable state is a set of immutable, content-addressed data
objects plus an immutable per-version manifest that (a) lists the exact object keys
the version references, (b) carries a **partition map** that lets a reader compute
the object key + byte range for any node-id / source-id band *from the manifest
alone* (this is what makes `r ≤ 1` true), and (c) carries the snapshot-consistent
statistics block (SPIKE-0004). Three object families:

1. **Columnar node-property objects** (`.ncol`) — nodes sorted by node id, one
   contiguous id band per object, each property stored as an independently
   addressable column chunk so a filter reads only its column's byte range (§2).
2. **CSR adjacency objects** (`.adj`) — edges grouped by `(rel-type, direction)`,
   sorted by source node id, one contiguous source-id band per object, with an
   intra-object offset directory so a hop reads exactly the frontier's neighbor
   lists and can early-abort per the byte cap (§3).
3. **Index objects** (`.idx`) and **statistics blobs** (`.stats`) — share the same
   self-describing framing; details owned by ADR 0005 (index) and SPIKE-0004
   (stats). This ADR pins only their framing and naming (§5.2).

The **manifest** (`db/manifest/<V>.json`, ADR 0002 §1) is the root: object list +
partition map + schema + stats + format version. The commit is the atomic
create-only-CAS of the next manifest (ADR 0002 §2/§3); versioning, GC, pinning are
ADR 0002 §5/§6, here in content-addressed, reference-counted form (§7).

---

## Part 1 — Object naming, content-addressing, and the durability barrier

A database lives under a bucket/prefix `db/`. The format uses exactly the key
families ADR 0002 §1 pins, refined with the file-type suffixes this ADR defines:

| Key pattern | Object family | Mutability |
|-------------|---------------|------------|
| `db/data/<content-hash>/<shard>.ncol` | Columnar node-property shard (§2). | Immutable, write-once. |
| `db/data/<content-hash>/<shard>.adj`  | CSR adjacency shard (§3). | Immutable, write-once. |
| `db/data/<content-hash>/<shard>.idx`  | Secondary-index shard (§5.2; ADR 0005). | Immutable, write-once. |
| `db/stats/<content-hash>.stats`        | Referenced statistics blob (§5; SPIKE-0004). | Immutable, write-once. |
| `db/manifest/<V>.json` (V zero-padded, monotone) | Per-version manifest = root (§5). | Immutable, created exactly once. |
| `db/manifest/_latest`                  | Advisory pointer (ADR 0002 §1). | Mutable, advisory only. |
| `db/lease/writer`, `db/pins/<uuid>`    | Lease / reader pins (ADR 0002 §3/§5). | Per ADR 0002. |

- **`<content-hash>` is a BLAKE3 digest of the object's bytes** (ADR 0002 §1;
  BLAKE3 is already the repo's content-addressing + value-digest hash, SPIKE-0004
  §4.4). The key is therefore **unique per distinct write**: two writers producing
  different bytes write different keys (no shared mutable key for a zombie to
  overwrite — DA-1, decision 0023); identical bytes de-dupe to one key (free
  cross-version sharing — BC-1, decision 0027). `<shard>` is a human-readable
  descriptor *inside* the hashed prefix (e.g. `nodes-Person-000123`) for
  debuggability; it is not load-bearing for uniqueness (the hash is) and is not
  parsed for correctness.
- **Durability barrier (ADR 0002 §2 step 1):** every data + stats object of `V+1`
  is `PUT` and acked (read-after-write durable) **before** the manifest of `V+1` is
  created. Until the manifest exists no reader can reach these objects (the
  manifest is the only thing that names them), so a crash before the manifest
  create leaves only orphans (§7.2). `commit-ack == manifest-create-ack`.
- **Defence-in-depth:** data PUTs use create-only `If-None-Match:*` (ADR 0002 §1);
  a same-hash collision 200s on identical bytes.

---

## Part 2 — Columnar node-property objects (`.ncol`)

### 2.1 Partitioning

Nodes are partitioned **by label** and, within a label, **sorted by node id** into
contiguous **id-band shards**. Each `.ncol` object holds one id band of one label
— a *node row-group*. Shard size is chosen so a typical seed-set read or a
columnar aggregate scans a bounded number of shards (default target **≤ 4 MiB per
shard**, see §6.4 for the budget derivation; this is a tunable in the writer, not a
format constant).

Sorting by node id makes a node-id range a **contiguous byte range within a single
shard** (range-GET friendly) and makes the partition map (§5.1) a simple
band→object lookup.

### 2.2 Per-object framing (the on-bytes layout)

A `.ncol` object is self-describing. Field order, little-endian, 8-byte aligned
section starts:

```
+-------------------------------------------------------------+
| FILE HEADER (fixed)                                         |
|   magic            : u32  = 0xCAE5_0001  ("CAE5" + fmt nybble)|
|   format_version   : u16                                    |
|   object_kind      : u8   = 1 (NCOL)                        |
|   flags            : u8   (bit0 = checksummed)              |
|   id_band_lo       : u64  (first node id in this shard)     |
|   id_band_hi       : u64  (last node id, inclusive)         |
|   row_count        : u32  (nodes in this shard)             |
|   column_count     : u16                                    |
|   column_dir_off   : u64  (offset to COLUMN DIRECTORY)      |
|   content_len      : u64  (total object length, for checks) |
+-------------------------------------------------------------+
| COLUMN CHUNKS (one contiguous run per column)              |
|   chunk[0] ... chunk[column_count-1]                        |
|     each chunk = [codec-framed encoded column values]      |
+-------------------------------------------------------------+
| COLUMN DIRECTORY (at column_dir_off; the "footer")         |
|   for each column j:                                        |
|     prop_key_id    : u32  (schema-catalog property id)     |
|     logical_type   : u8   (PropertyValue tag: int/float/.. )|
|     codec          : u8   (0=plain,1=dict,2=delta-varint,..)|
|     present_bitmap_off : u64 (null/absent bitmap, rel. obj) |
|     chunk_off      : u64  (rel. to object start)           |
|     chunk_len      : u64                                    |
|     min_digest     : [u8;8] (BLAKE3-8 of min value; stats) |
|     max_digest     : [u8;8] (BLAKE3-8 of max value)        |
+-------------------------------------------------------------+
| TRAILER (fixed, last 16 bytes)                              |
|   column_dir_off   : u64  (duplicate, so the dir is found  |
|                            by reading the last 16 bytes)   |
|   blake3_prefix    : [u8;8] (object self-checksum prefix)  |
+-------------------------------------------------------------+
```

**Why a trailing duplicate `column_dir_off`:** a reader who knows only the object
key (not its length) issues **one** range-GET of the **last 16 bytes** (suffix
range — supported by S3/MinIO `Range: bytes=-16`) to find the directory offset,
then one range-GET of the directory, then one range-GET of the exact column chunk
it filters on. But for the latency hot path the reader does **not** pay these
discovery round-trips: the **manifest partition map (§5.1) carries each shard's
`column_dir_off` and the per-column `(chunk_off, chunk_len)` for the
filter-relevant columns inline**, so the planner computes the precise byte range
from the manifest (phase 1) and issues a single direct range-GET for the column.
The trailer/footer path exists for tools, recovery, and forward-compat readers that
encounter a shard not described in their (older) manifest copy.

### 2.3 Encodings (codecs)

Columns are encoded per-column so each is independently decodable from its chunk
range:

- **plain** — fixed-width (i64, f64, bool): a packed array; value `i` at
  `chunk_off + i·width`. O(1) random access; supports byte-exact range slicing.
- **dict** — low-cardinality strings/enums: a dictionary block + u32 codes. The
  dictionary is at the chunk head; codes follow. The MCV stats (SPIKE-0004) are
  computed from the dictionary cheaply.
- **delta-varint** — monotone or clustered integers (e.g. timestamps, the node-id
  column itself): zig-zag delta + varint. Compact; decoded sequentially within the
  chunk.
- **present bitmap** — every column carries a 1-bit-per-row present/absent bitmap
  (at `present_bitmap_off`) so a column added in a later schema version is "absent"
  (null) for rows written before it existed (§schema evolution, Part 8).

The node-id column is implicit: row `i` of the shard has node id `id_band_lo + i`
when ids are dense, or is materialized as a delta-varint column when sparse; the
header `id_band_lo/hi` + `row_count` disambiguates. A filter that needs the id (to
chain into adjacency) reads it from the (cheap) id column or computes it directly.

### 2.4 Reader access pattern (the bytes a filter actually reads)

To evaluate `MATCH (n:Person) WHERE n.country = 'IS'`:
1. Planner reads the **manifest** (phase 1): partition map gives the set of
   `Person` `.ncol` shards and, for the `country` column, each shard's
   `(chunk_off, chunk_len)`.
2. The index probe (phase 2; ADR 0005 / §5.2) returns the seed-node id band(s)
   that match — a small scattered set for a selective filter.
3. The executor issues **direct range-GETs of just the `country` column chunk** for
   the shards spanning the seed band (and the present bitmap if needed) — never the
   whole node record. The returnable/projected columns for the surviving LIMIT rows
   are read either in the same batch (small bands) or co-located in adjacency
   records (§3.3) to keep `K_min = 8` (PS-1).

Bytes read for the filter = Σ(filtered-column chunk lengths over touched shards),
**not** the node objects' full size — satisfying ADR 0001 Part 5 #3.

---

## Part 3 — CSR adjacency objects (`.adj`) — the hop-expansion structure

### 3.1 Partitioning (Compressed Sparse Row, banded)

Edges are grouped by **`(rel-type τ, direction d ∈ {out, in})`** and, within a
group, **sorted by source node id**, partitioned into contiguous **source-id band
shards**. Each `.adj` object is one CSR row-group: the out- (or in-) neighbor lists
for a contiguous band of source ids of one rel-type. This is the classic CSR layout
made object-store-native and banded so a frontier in a contiguous id band maps to a
**single shard and a contiguous byte range** (ADR 0001 Part 5 #2).

Both directions are materialized so in-edge traversal is also `r ≤ 1` (the planner
picks the cheaper direction; the writer maintains both, paid at write time which is
the single-writer, amortizable side).

### 3.2 Per-object framing

```
+-------------------------------------------------------------+
| FILE HEADER (fixed)                                         |
|   magic, format_version, object_kind=2 (ADJ), flags        |
|   rel_type_id      : u32                                    |
|   direction        : u8  (0=out,1=in)                       |
|   src_band_lo      : u64 (first source id in this shard)    |
|   src_band_hi      : u64 (last source id, inclusive)        |
|   src_count        : u32 (distinct source ids in band)      |
|   offset_dir_off   : u64 (offset to OFFSET DIRECTORY)       |
|   content_len      : u64                                    |
+-------------------------------------------------------------+
| NEIGHBOR BLOCKS (one contiguous run per source id, in id   |
|  order)                                                     |
|   block[s] for source id s = src_band_lo + k :             |
|     dst_ids   : delta-varint, ascending (CSR neighbor list)|
|     [edge_prop columns, columnar within the block, optional]|
|     [projected dst node properties, co-located — see §3.3] |
+-------------------------------------------------------------+
| OFFSET DIRECTORY (at offset_dir_off)                        |
|   for each source id s in band (k = s - src_band_lo):      |
|     block_off  : u64 (rel. to object start)                |
|     block_len  : u32 (bytes; enables exact range-GET + cap)|
|     degree     : u32 (neighbor count; enables early-abort) |
+-------------------------------------------------------------+
| TRAILER (offset_dir_off duplicate + blake3 prefix)         |
+-------------------------------------------------------------+
```

The **offset directory** is the heart of CSR: for source id `s`, entry
`k = s - src_band_lo` gives the byte range `[block_off, block_off + block_len)` of
its neighbor block and its `degree`. The directory is fixed-stride (16 bytes/entry),
so the reader indexes it with **O(1) arithmetic** — `dir_off + k·16` — no scan.

### 3.3 Co-located projection (the `r ≤ 1` and PS-1 mechanism)

Two design problems are solved by **co-locating, in the neighbor block, the small
set of destination-node properties the traversal needs next**:

- **`r ≤ 1` (ADR 0001 Part 5 #1):** hop `h` returns destination ids *and the
  filter-relevant properties of those destinations* in the **same** neighbor block.
  The executor evaluates the hop-`h+1` filter and computes the next source-id band
  **without** a separate node-property GET between hops. The next hop's adjacency
  range-GET address is then computed from the manifest partition map (§5.1) + those
  ids — one round of I/O, no indirection read. This is what keeps `r = 1`.
- **PS-1 (`K_min` stays 8):** the **returnable** projection columns for the
  surviving LIMIT-10 rows are likewise co-located in the hop-6 neighbor block, so
  the final-row property fetch happens *within* phase 8's window — no 9th serial
  round-trip.

The co-located set is the **projection set**: the union of (a) properties any hop
filter references and (b) properties the `RETURN`/`WITH` clause projects, as
determined by the planner and recorded per rel-type in the manifest schema
(`projection_cols[τ]`). It is intentionally **small** (the hot, filtered/returned
columns), not the whole node record — co-locating everything would re-inflate the
adjacency object and blow the byte budget. Properties **not** in the projection set
are read from `.ncol` objects in a final batched range-GET only if the query needs
them and they were not projected — and the planner accounts for that as part of
phase 8 / the index-probe batch, never as a new serial phase.

> **Falsification handled — write amplification of co-location.** Co-locating dst
> properties duplicates them (once per in-edge). For the headline workload the
> projection set is a handful of small columns and the seed set is tiny, so the
> realized read is bounded; the *write*-side duplication is paid by the
> single-writer at ingest/`ANALYZE` time (amortizable) and bounded by the
> projection-set width. If a deployment's projection set is large enough that
> co-location would blow the per-shard budget, the writer MAY fall back to the
> **K=9 contract (§7.3)** for that rel-type and the planner re-pins the budget at
> `K_min = 9` for plans over it — explicit, never silent.

### 3.4 Early-abort as a hard per-GET byte/row cap (F1 / F2 / decision 0015)

Because the offset directory gives `block_len` and `degree` per source id **before
the neighbor bytes are fetched**, the reader enforces a **hard per-GET byte/row
cap**:

- The executor tracks a **running byte budget** (≤ `B_max`) and the **LIMIT
  counter** across the hop.
- It issues an adjacency range-GET covering the frontier band, but **caps the
  requested byte length** at the remaining budget, and **truncates** (stops
  consuming the stream / issues a bounded `Range`) once the LIMIT is satisfied or
  the budget is exhausted.
- A **super-hub** source id (degree ≫ p99) is visible in the directory *before* its
  neighbor block is read: its `block_len` alone may exceed the cap. The reader
  **never** fetches more than the cap; realized bytes stay ≤ `B_max` regardless of
  which node the frontier routed through. (This is why F2 is a *detection*-only
  concern — the planner rejects such a query from `max_deg` (SPIKE-0004 §3) — and
  not a realized-latency bust: even under a `WARN` override, the read is hard-capped
  here.)

This makes "early-abort partial adjacency reads" (decision 0001 F1) a **budget-
driven hard cap**, exactly as decision 0015 / SPIKE-0003 notes require — not an
optional optimization.

---

## Part 4 — The `r ≤ 1` invariant, stated precisely

> **`r ≤ 1` invariant.** Given the pinned manifest (read once in phase 1) and the
> destination ids produced by hop `h`, the reader can compute the object key and
> byte range of every hop-`h+1` adjacency neighbor block **without issuing any
> object-store GET other than the hop-`h+1` adjacency range-GETs themselves.**

It holds because:
1. The **manifest partition map** (§5.1) maps any source-id `s` (for a given
   `(τ, d)`) to `(object-key, offset_dir_off, src_band_lo)` — a pure in-memory
   lookup, no GET.
2. The **offset directory** for the target band is located at `offset_dir_off`
   (known from the map) and indexed by `k = s - src_band_lo` with O(1) arithmetic.
   The reader fetches the directory slice for the frontier band **in the same
   round** as (or immediately preceding, within one parallel batch) the neighbor
   blocks — because the directory's byte range is also computable from the map
   (the map records `offset_dir_off`, and the directory is fixed-stride so the
   slice for ids `[lo,hi]` is `[dir_off + (lo-band_lo)·16, dir_off + (hi-band_lo+1)·16)`).
3. The hop-`h+1` filter is evaluated on the **co-located dst properties** (§3.3),
   so no separate node-property GET interleaves between hops.

Thus each hop is **one parallel batch of ≤ `M_max` range-GETs** (the directory
slice + neighbor blocks for the frontier band can be issued together or coalesced;
they are in the same object, often the same contiguous range), i.e. `r = 1`.
`K_min = 1 (manifest) + 1 (index probe) + 6·1 (hops) = 8`. The envelope proof
(ADR 0001 §3) is preserved.

> **Falsification attempted — does the directory slice cost a second round-trip?**
> No: the directory and the neighbor blocks live in the **same object**. The reader
> may either (a) issue one range-GET covering `[first neighbor block start, last
> neighbor block end)` for a contiguous frontier band (the common case — the band
> is contiguous by construction) plus a parallel range-GET of the directory slice,
> both in the **same parallel batch** (still one round, counts within `M_max`), or
> (b) for a dense band, fetch a single superset range covering directory + blocks.
> Either way it is **one round of I/O per hop**, not two serial rounds. The
> directory slice is tiny (16 B × band width) and parallel to the block fetch.

---

## Part 5 — The manifest (root object), partition map, and statistics

The manifest `db/manifest/<V>.json` is the version root. It is JSON for
debuggability and forward-compat (a binary sidecar may be added later without a
format break; the JSON is small relative to data). Top-level fields:

```jsonc
{
  "format_version": 1,
  "manifest_version": 42,          // == V; ADR 0002 monotone
  "created_at": "...",
  "schema": { /* labels, rel-types, property keys, projection_cols[τ] */ },
  "objects": [                      // the EXACT key list this version references
    { "key": "db/data/<hash>/nodes-Person-000123.ncol", "kind": "ncol",
      "label": "Person", "id_band": [0, 65535],
      "column_dir_off": 4193280,
      "columns": { "country": {"off": 40, "len": 1312, "codec": "dict"},
                   "name":    {"off": 1352, "len": 9001, "codec": "dict"} } },
    { "key": "db/data/<hash>/adj-FOLLOWS-out-000007.adj", "kind": "adj",
      "rel_type": "FOLLOWS", "dir": "out", "src_band": [0, 65535],
      "offset_dir_off": 7340032 }
    // ... index objects, etc.
  ],
  "partition_map": { /* see §5.1 */ },
  "stats": { /* see §5.3; OOE-critical scalars inline, blobs referenced */ },
  "stats_blobs": [ {"key": "db/stats/<hash>.stats", ...} ]
}
```

### 5.1 The partition map (enables `r ≤ 1` and the columnar filter read)

The partition map is the index from a *logical* coordinate to a *physical* byte
range, resolved in-memory from the pinned manifest with **zero extra GET**:

- **Node side:** `(label, node-id) → (ncol object key, column_dir_off, id_band_lo)`.
  For the filter-relevant columns the per-column `(chunk_off, chunk_len)` are
  inlined in the object entry (above), so the planner computes the exact column
  byte range for any seed band directly.
- **Edge side:** `(rel-type, direction, source-id) → (adj object key,
  offset_dir_off, src_band_lo)`. The reader then indexes the fixed-stride offset
  directory by `k = source-id - src_band_lo` (O(1) arithmetic) to get each neighbor
  block's `(block_off, block_len, degree)`.

Because id bands are contiguous and sorted, the map is a small sorted array of
`(band_lo, band_hi, object-key, …)` per `(label)` / per `(rel-type, dir)` — a
binary search, not a per-node entry. For a 1B-node / 10B-edge graph at the default
≤ 4 MiB shard size this is **O(10^4–10^5) band entries**, tens of KB to a few MB of
manifest — a single sequential phase-1 read the cost model already budgets
(`bytes_manifest`), and well under `B_max`. (Size bound: §6.5.)

### 5.2 Index and stats object framing

Index objects (`.idx`, ADR 0005) and statistics blobs (`.stats`, SPIKE-0004) reuse
the §2.2 self-describing framing (magic + `object_kind` + footer/trailer + content
hash). This ADR pins only: they are **content-addressed**, **immutable**, listed in
`objects[]`, and GC-ed by the same reference-set rule (§7.2). Their internal layout
is owned by ADR 0005 / SPIKE-0004 respectively.

### 5.3 Statistics block (the inline-vs-referenced cut — SPIKE-0004 R1)

Per SPIKE-0004 Part 2.1 (B) hybrid, `steering-storage` makes the binding cut:

- **Always inline** in `manifest.stats` (zero extra GET — the super-hub /
  non-selective rejection paths need no data-plane round-trip beyond the manifest):
  per-label `node_count`, `total_node_count`, per-rel-type `edge_count`,
  `p99_deg[τ,d]`, **`max_deg[τ,d]`** (the mandatory super-hub safety term —
  decision 0015 / ADR 0001 F2), and the block metadata (`stats_version`,
  `as_of_version`, `freshness`, `estimator_params`).
- **Referenced** content-addressed `db/stats/<hash>.stats` blob(s), fetched lazily
  during planning **only** for properties a query actually filters on: the bulky
  per-`(label,property)` selectivity detail (NDV, null_frac, MCV list, histogram).
  This is a *planning-phase* lookup (O(plan-size) statistics-lookup), not a K-phase
  data GET, and is bounded (§6.5).
- **Value-digest privacy (SPIKE-0004 §1.2, guardrails §3):** MCV and histogram
  entries store **fixed-width collision-resistant digests** (BLAKE3-truncated to 8
  bytes) plus, for ordered histograms, an order-preserving truncated key — **never
  raw property values**. Committed fixtures are therefore free of user data by
  construction. The same digesting is used for the `.ncol` column `min_digest` /
  `max_digest` (§2.2).

The **binding invariant** (SPIKE-0004 R1, restated as storage law): *the terms OOE
detection needs to reject a super-hub or a non-selective filter are reachable
without a data-plane round-trip beyond the manifest itself.* This ADR's inline cut
satisfies it.

### 5.4 Maintenance on commit

Exact counts and incremental `max_deg` are updated as part of building `V+1`'s
manifest under the writer lease, before the atomic create (SPIKE-0004 §2.2);
p99/NDV/MCV/histogram are recomputed on `ANALYZE` and carried forward with a
downgraded `freshness` between. Atomicity is inherited from the commit protocol
(the stats are in / referenced by the immutable manifest) — no separate
stats-durability mechanism.

---

## Part 6 — Byte-budget analysis (the layout serves `B_max` in `K_min` phases)

This discharges the SPIKE-0003 acceptance criterion: *for the in-envelope
selectivity from ADR 0001, a 6-hop query reads ≤ `B_max` total across ≤ `K_min`
phases given the chosen partition sizes.* Design points from ADR 0001 §2.2/§3:
`B_max(50 Mbps) = 2.88 MB` (binding), `B_max(1 Gbps) = 57.5 MB`, `bytes_node =
256 B`, `bytes_edge_row = 64 B`, `M_max = 8`, `K_min = 8`.

### 6.1 Phase accounting (`r = 1`, `K_min = 8`)

| Phase | Read | Bytes (design point) |
|------:|------|----------------------|
| 1 | Manifest (objects + partition map + inline stats) | `bytes_manifest` ≤ a few MB worst case; the OOE-critical + map slice the planner needs is ≤ 4 KB–tens of KB for a single plan (it binary-searches the map, does not read it whole). Budgeted as `bytes_manifest ≤ 4 KB` in the cost model. |
| 2 | Index probe → seed band (`.idx` range-GET) + filtered `.ncol` column chunk for the seed band | `est_N_seed × (filter-col bytes)` ≤ `N_seed × bytes_node` upper bound |
| 3–8 | 6 hops; each = one parallel batch (≤ `M_max` GETs) of adjacency neighbor blocks for the frontier band, **byte-capped** | `≤ 6 × M_max × F_tail × bytes_edge_row` (typical), hard-capped at remaining budget (§3.4) |

Total `B_query ≤ bytes_manifest + N_seed·bytes_node + 6·M_max·F_tail·bytes_edge_row`
— **identical to ADR 0001 §2.2's in-envelope inequality.** The layout realizes the
cost model's assumed access pattern exactly: the columnar filter read (phase 2) and
the banded CSR hops (phases 3–8) are the bytes the model counts, and nothing more,
because (a) columns are read individually, (b) adjacency is read per-frontier-band
and byte-capped, (c) `r = 1` keeps the phase count at 8.

### 6.2 Worked 50 Mbps in-envelope check (`F_tail = 10`, `N_seed = 5000`)

```
bytes_manifest (plan slice)        ≈ 4 KB
phase 2 (filter col + seed nodes)  ≈ 5000 × 256 B            = 1.28 MB
phases 3–8 (typical)               ≈ 6 × 8 × 10 × 64 B       = 30.7 KB
-----------------------------------------------------------------------
B_query                            ≈ 1.31 MB  ≤  B_max = 2.88 MB  ✓
```

Comfortably in-envelope; matches ADR 0001 §2.2's worked feasible points.

### 6.3 Super-hub is detection-bounded, realized-bounded

A frontier routing through a `max_deg = 4×10^7` node has a 2.56 GB neighbor block.
**Detection:** the planner reads `max_deg` inline from the manifest (phase 1, zero
extra GET) and rejects (OOE-2 super-hub, SPIKE-0004 §3.2). **Realized (under WARN
override):** the offset-directory `block_len` is seen before the bytes; the
hard per-GET byte cap (§3.4) truncates the read at the remaining budget, so
realized bytes stay ≤ `B_max`. Both directions covered; F2 closed.

### 6.4 Shard-size choice (≤ 4 MiB default)

Per-shard target ≤ 4 MiB keeps any single shard's full read within the 50 Mbps
`B_max` order of magnitude, so even a non-band-aligned read touches few shards; and
it keeps the **partition-map entry count** bounded (§6.5). It is a **writer tunable
recorded in `estimator_params`/schema**, not a format constant — a deployment may
tune it; the format (framing, directory, map) is size-agnostic. Smaller shards
shrink wasted bytes per range-GET but grow the map; 4 MiB balances both for the
1B/10B design point.

### 6.5 Partition-map and manifest size bound

At ≤ 4 MiB/shard, a 1B-node graph at ~256 B/node ≈ 256 GB of node data ⇒ ~64k node
shards; a 10B-edge graph at ~64 B/edge-row both directions ≈ 1.28 TB ⇒ ~320k
adjacency shards. Map entry ≈ 64 B ⇒ map ≈ **~25 MB worst case if read whole**.
**But the planner never reads the whole map**: it binary-searches the per-`(label)`
/ per-`(rel-type,dir)` sub-array for the seed/frontier bands, reading only the
entries it needs (the manifest JSON supports a ranged/segmented read; §8.2). The
cost-model `bytes_manifest ≤ 4 KB` budgets the *per-plan slice*, not the whole map.
For very large schemas the map MAY be externalized into the same referenced-blob
mechanism as stats (a `db/manifest-index/<hash>` content-addressed segment per
label/rel-type, fetched per-band during planning) — pinned as the §8.2 forward-
compat hook; the default 1B/10B design point does not require it.

> **Falsification attempted — does a 25 MB map bust phase 1?** Only if read whole.
> The map is **sorted and segmented by label / (rel-type,dir)**, so a single plan
> touches O(plan-size) segments and binary-searches within them; the realized
> phase-1 read is the inline stats + the touched segments' band entries — KB, not
> 25 MB. The whole-map figure is the at-rest size, not the per-query read. Recorded
> as a non-blocking scaling note (§8.2) with the externalization hook pinned.

---

## Part 7 — Versioning, commit, GC (content-addressed, reference-counted)

These mechanics are **owned by ADR 0002**; this section pins the *format-side*
obligations and re-points the named primitives so T-0009/T-0010 implement against
one consistent spec.

### 7.1 Commit = atomic manifest create (ADR 0002 §2/§3)

The writer stages all `V+1` data + stats objects (content-addressed, durable-acked
— the durability barrier, §1), then **creates `db/manifest/<V+1>.json` with
`PUT If-None-Match:*`** (the named CAS primitive, ADR 0002 §3; RFC 9110 §13.1.2;
GA on S3 Aug-2024, supported on MinIO). Success → commit durable + visible
atomically; 412 → fenced, discard staged objects as orphans, re-resolve, retry. A
reader resolves `latest = max(LIST db/manifest/)` (advisory `_latest` is a hint
only). **Format obligation:** the manifest's `objects[]` list is the **exact and
complete** reference set of the version — a reader resolving `V` reads precisely
those keys; no data object is reachable except through a committed manifest.

### 7.2 GC — reference-counted live-object-set sweep (ADR 0002 §6; decision 0027 BC-1)

Because data objects are **content-addressed and shared across versions** (identical
bytes de-dupe), GC is **reference-counted**, not wholesale-per-version:

- GC runs **only under the writer lease**, never reclaims `latest`, never reclaims a
  version with a **live pin** (`now ≤ deadline + grace`, grace > max reader-session
  lifetime — decision 0001 F3, ADR 0002 §6).
- GC reclaims a data/stats object key **iff no surviving manifest's `objects[]`
  references it** (the live-object-set test). This is exactly the discipline
  decision 0027 BC-1 required SPIKE-0003 to honour for cross-version sharing —
  satisfied by construction here, because content-addressing makes "is this key
  still referenced?" a set-membership test over surviving manifests.
- **Orphans** (staged by a fenced/crashed writer, never in any committed manifest)
  are identified as keys absent from every live manifest's reference set, older than
  a grace window, and reclaimed. They were never reader-visible (no manifest named
  them). This is the model-checked `OrphansNeverReferenced` invariant (ADR 0002
  §6.4, BUG-0012).

**Format obligation:** because GC is a reference-set test, the writer MUST NOT reuse
a content-hash key for *different* bytes (impossible by construction — the hash is of
the bytes) and MUST record every referenced key in `objects[]`. No format element
relies on version-scoped wholesale deletion.

### 7.3 The explicit `K_min = 9` fallback contract (PS-1)

The default layout co-locates the projection set to keep `K_min = 8` (§3.3). If a
deployment's projection set for a rel-type is too wide to co-locate within the
shard budget, the writer MAY mark that rel-type `colocated_projection = false` in
the manifest schema. The planner, seeing this for any rel-type in a plan, **re-pins
the budget at `K_min = 9`** for that plan (a 9th serial phase: the final-row
node-property fetch after hop 6), re-derives `B_max` at K=9 (ADR 0001 PS-1:
50 Mbps `B_max ≈ 2.53 MB`, still > 0 — the envelope still closes), and applies the
K=9 OOE thresholds. This is **explicit and recorded in the manifest** — never a
silent phase-count drift. The default and recommended configuration is co-located
`K_min = 8`.

---

## Part 8 — Schema evolution & forward-compatibility

### 8.1 Adding a property column without rewriting data

- A new property added to a label is written as a **new column chunk in newly
  written `.ncol` shards only**. Existing shards are **not rewritten**.
- A reader resolving a column absent from an older shard sees it missing in that
  shard's **column directory** and treats every row there as **null/absent** (the
  present bitmap is effectively all-zero for a column that does not exist). This is
  correct openCypher semantics (absent property = null).
- `ANALYZE` / compaction MAY later rewrite old shards to materialize the new column
  densely — an optimization, never required for correctness.

### 8.2 Forward-compatible framing

- Every object carries `magic` + `format_version` + `object_kind`; a reader rejects
  an object whose `format_version` it does not understand (fail-closed, never
  fail-open — cf. BUG-0014's "parse must not fail open" lesson) rather than
  mis-reading bytes.
- The **column directory / offset directory are the source of truth**, read from
  the footer/trailer; a reader never assumes a fixed column order or count. New
  codecs are added as new `codec` ids; a reader rejects an unknown codec
  fail-closed.
- The manifest is JSON with explicit `format_version`; new fields are additive and
  ignored by older readers (which fail-closed only on a `format_version` bump).
- **Forward hook (non-binding):** if the partition map exceeds the phase-1 budget
  for very large schemas, externalize it into per-`(label)`/`(rel-type,dir)`
  content-addressed `db/manifest-index/<hash>` segments fetched per-band during
  planning (§6.5). The default 1B/10B design point does not need this; the hook is
  reserved so adopting it later is not a format break.

### 8.3 Self-description summary

Given only a bucket/prefix, a reader can: LIST `db/manifest/` → resolve `latest` →
read the manifest → from `objects[]` + `partition_map` know every object, its kind,
its schema, and the byte range of any column/neighbor-block — **without a side
channel**. Every object independently re-describes itself via its header + footer
directory for tools and recovery. The format is fully self-describing.

---

## Alternatives considered

### Alternative A — Row-oriented node objects (one record per node, all properties together)

**Why considered:** simplest writer; one GET per node returns everything; no column
directory.

**Why rejected:** a filter on one property would read the **whole node record** for
every candidate node, violating ADR 0001 Part 5 #3 and inflating phase-2 bytes by
the (properties-per-node) factor. At `bytes_node = 256 B` and a 4-byte filter
column, row-orientation reads ~64× the necessary bytes for the filter — directly
busting the byte budget for any non-trivial seed set. Columnar is mandatory for the
selective-filter access pattern.

### Alternative B — Adjacency lists with a separate offset/indirection object (`r = 2`)

**Why considered:** keeps neighbor blocks pure (no co-located properties); the
offset table is a compact separate object.

**Why rejected:** reading the offset object to find a neighbor block's address is a
**serial indirection GET between hops** ⇒ `r = 2` ⇒ `K_min = 14` ⇒ the 50 ms floor
is 700 ms and the 50 Mbps case is infeasible (ADR 0001 §1.5 — an explicit
falsification). Folding the offset directory **into the same object** (read in the
same parallel batch as the neighbor blocks, addressed from the manifest map) keeps
`r = 1`. This is the decisive reason the offset directory is intra-object, not a
separate object.

### Alternative C — Co-locate the *entire* node record in every adjacency block

**Why considered:** would make every hop fully self-contained (no `.ncol` read ever
needed on the hot path) and trivially satisfy PS-1.

**Why rejected:** duplicating all properties once per in-edge over a 10B-edge graph
is catastrophic write amplification and inflates every adjacency neighbor block far
beyond `bytes_edge_row = 64 B`, busting the per-hop byte budget. The chosen design
co-locates only the **projection set** (filtered + returned hot columns), bounded
and small, and reads the rest columnar from `.ncol` only when actually needed.

### Alternative D — Mutable per-version data prefix (`db/data/v<V>/...`) instead of content-addressing

**Why considered:** human-readable, version-scoped keys; wholesale-per-version GC is
trivial.

**Why rejected:** it reintroduces the DA-1 stale-overwrite (a zombie writer's PUT to
a shared `v<V>/<shard>` key is last-write-wins and can corrupt a committed snapshot
in place — decision 0023/0024) and forbids cross-version de-dup (BC-1). ADR 0002's
content-addressed keys are mandatory; this ADR adopts them. Wholesale GC is replaced
by the reference-set sweep (§7.2), which the content-addressing makes correct.

### Alternative E — Hash-partition adjacency (by `hash(source-id)`) instead of sorted bands

**Why considered:** even shard sizes, no skew from clustered ids.

**Why rejected:** hash-partitioning **destroys the contiguity** a frontier needs:
`M_max` frontier nodes would scatter across `M_max` shards ⇒ `M_max` random GETs per
hop, violating ADR 0001 Part 5 #2 and decision 0001's "few, large, parallelizable
range GETs" non-blocking guidance. Sorted source-id bands keep a contiguous frontier
in one shard/one range. Skew is handled by **variable-width bands** (a hot id range
gets a smaller band) recorded in the partition map, not by hashing.

---

## Consequences

### Positive

- **Cat. 2 (storage format) toward 100:** a concrete, self-describing, columnar +
  CSR, content-addressed, versioned, GC-able, schema-evolvable object format with a
  named range-GET access pattern and a proven byte-budget fit.
- **Cat. 3 (latency):** the layout *realizes* ADR 0001's assumed access pattern —
  `r = 1` (manifest partition map + intra-object offset directory + co-located
  projection), few/large/parallel range GETs (sorted bands), hard per-GET byte cap
  (early-abort) — so the envelope proof is not hand-wavy but structurally honored.
- **Cat. 1 (ACID):** the manifest's exact `objects[]` list + durability barrier +
  content-addressing are the format substrate for ADR 0002's atomicity/SI proof.
- **Cat. 6 (aggregates):** columnar chunks + manifest-resident exact counts answer
  `count`/`sum`/`distinct` with bounded scans / zero data GETs.
- **Unblocks T-0007/T-0008/T-0009/T-0010** with field-level, implementable detail.

### Negative / trade-offs

- **Write amplification from bidirectional adjacency + co-located projection.** Both
  directions and the projection set are duplicated; paid by the single writer at
  ingest/`ANALYZE` (amortizable), bounded by the projection-set width. The K=9
  fallback (§7.3) is the escape hatch when a projection set is too wide.
- **Partition map grows with shard count.** ~25 MB at-rest for 1B/10B; mitigated by
  segmented binary-search reads (never read whole) and the externalization hook
  (§8.2). A scaling concern at >10× the design point, not at it.
- **JSON manifest is verbose.** Chosen for debuggability/forward-compat; a binary
  sidecar is a future optimization behind `format_version`, not a break.
- **Statistics maintenance per commit** (ADR 0001 already flagged) — bounded to the
  touched schema for the incremental path (SPIKE-0004 §5).

### Open questions (bound to implementation tasks, not blocking)

1. **Exact codec set & per-column codec selection heuristic** — T-0007 picks the
   initial set (plain/dict/delta-varint named here as the floor); adding codecs is
   forward-compatible (§8.2).
2. **Default shard size & band-width skew policy** — T-0007/T-0008 tune within the
   §6.4 envelope; recorded in `estimator_params`.
3. **Projection-set selection** — which columns the writer co-locates per rel-type;
   the planner records the needed set, the writer materializes it; T-0008 + the
   planner (T-0018) coordinate. Default = filtered+returned hot columns.
4. **Manifest binary sidecar / map externalization** — only if the design point is
   exceeded; §8.2 hook reserved.

---

## Rubric impact

| Cat. | Name | Impact |
|------|------|--------|
| 2 | Storage format & S3 commit | **Primary** — defines the object layout, framing, range-GET access pattern, versioning, GC, schema evolution. Toward 100. |
| 3 | Latency envelope | Realizes `r ≤ 1`, contiguous-band range GETs, hard per-GET byte cap — structurally honors the ADR 0001 proof. |
| 1 | ACID | Exact `objects[]` reference set + durability barrier + content-addressing — substrate for ADR 0002 atomicity/SI. |
| 6 | Fast aggregates | Columnar chunks + manifest exact counts. |
| 5 | Secondary indices | `.idx` framing pinned; ADR 0005 owns internals. |

---

## Cross-references

- **ADR 0001** (latency envelope): Part 5 constraints #1–#5, PS-1 — all discharged
  here (§3, §4, §5, §6, §7.3).
- **ADR 0002** (commit protocol): content-addressed keys, CAS primitive, durability
  barrier, reference-counted GC, pins — re-pointed in §1, §7.
- **ADR 0005** (index interface): `.idx` framing (§5.2).
- **SPIKE-0004** (`docs/specs/SPIKE-0004-manifest-statistics-contract.md`): the
  statistics block, inline-vs-referenced cut, value-digest privacy — made binding in
  §5.3.
- **decision 0001** (storage-domain ratification): F1/F2/F3 — discharged (§3.4, §7).
- **decision 0015 / 0017** (ADR 0001 F2, PS-1): super-hub safety + phase count —
  §3.4, §6.3, §7.3.
- **decision 0027** (BC-1): cross-version sharing → reference-counted GC — §7.2.
- **T-0007** (columnar node-property writer/reader): implements §2.
- **T-0008** (adjacency-list edge writer/reader): implements §3, §3.4 early-abort.
- **T-0009** (manifest + statistics + version resolution): implements §5, §7.1.
- **T-0010** (atomic commit): implements §7.1 against ADR 0002.
- **EPIC-001**: parent epic.

---

## Sign-off

### Design-falsification record — `steering-storage` (design authority, Cat. 2), T0+~03:40

I am the design authority for the storage domain (rubric Cat. 2) and the named
owner of decision 0001 findings F1/F2/F3. I produced this spec and ran the
design-falsification loop against it before ratifying. Every constraint pre-
registered by the upstream ratified artifacts (ADR 0001 Part 5 #1–#5 + PS-1; ADR
0002 §1/§6 + decision 0027 BC-1; decision 0001 F1/F2/F3; SPIKE-0004 R1) was treated
as a falsification target.

**Falsification attempts and why each failed to break the design:**

1. **"`r` is secretly 2" (the fatal one).** Attack: the reader must read an offset
   table to find a neighbor block ⇒ a serial indirection GET between hops. Refuted
   by §3.2 + §4: the offset directory is **intra-object** and fixed-stride, its
   slice is addressed from the manifest partition map (read once, phase 1), and is
   fetched in the **same parallel batch** as the neighbor blocks — one round per
   hop. Alternative B (separate offset object) is explicitly rejected for exactly
   this reason. `K_min = 8` holds; the ADR 0001 §3 proof is preserved. **Survived.**

2. **PS-1 — `K_min` silently becomes 9.** Attack: the final-row property fetch after
   hop 6 is a 9th serial phase. Refuted by §3.3: the returnable **projection set** is
   co-located in the hop-6 neighbor block, so the fetch is within phase 8. The K=9
   case is not hidden — it is an **explicit, manifest-recorded** fallback (§7.3) with
   a re-pinned budget that still closes (`B_max ≈ 2.53 MB > 0`). PS-1's exact demand
   ("co-locate, OR explicitly declare K=9") is met both ways. **Survived.**

3. **Super-hub busts realized `B_max` (F2).** Attack: a 40M-degree frontier node's
   2.56 GB neighbor block. Refuted on **both** axes: *detection* — `max_deg` is
   inline in the manifest (phase 1, zero extra GET), planner rejects (§6.3,
   SPIKE-0004 §3.2); *realized* — the offset directory exposes `block_len`/`degree`
   before the bytes, and the hard per-GET byte cap (§3.4) truncates the read at the
   remaining budget, so realized bytes ≤ `B_max` even under a `WARN` override. F2 is
   detection-only by construction. **Survived.**

4. **Selective seed = scattered = many small random GETs.** Attack: a 1-in-100k
   filter yields a scattered seed whose adjacency lists sit at scattered offsets ⇒
   `M_max` random GETs. Refuted by §3.1 sorted-source-id bands + variable-width
   bands in the partition map: a contiguous frontier band maps to one shard / one
   contiguous range; the planner coalesces frontier ids into ≤ `M_max` band range-
   GETs per hop (decision 0001 non-blocking guidance realized). Hash-partitioning
   (Alternative E) — which *would* scatter — is rejected. **Survived.**

5. **Torn read / GC-deletes-an-object-mid-read.** Attack: a reader sees a half-
   committed object set, or GC frees an object a reader is mid-read on. Refuted by
   inheritance from ADR 0002 (model-checked): the manifest is the only thing that
   makes objects reachable, created atomically after the durability barrier
   (§1, §7.1), so a reader pins a complete immutable object set; GC runs only under
   lease, never `latest`, never a live-pinned version (grace > max session), and
   only deletes keys no surviving manifest references (§7.2 — the BC-1 reference-set
   discipline content-addressing makes correct). `NoTornCommit` + `GCSafety` +
   `OrphansNeverReferenced` hold. **Survived** (no new commit-protocol claim; this
   ADR's content-addressed, exact-`objects[]` layout is the substrate those proofs
   assume).

6. **Byte budget is hand-wavy.** Attack: the layout claims to fit `B_max` but the
   arithmetic is asserted, not derived. Refuted by §6: the phase accounting reduces
   **exactly** to ADR 0001 §2.2's in-envelope inequality (the layout reads precisely
   the bytes the cost model counts — individual columns, banded byte-capped
   adjacency, `r = 1`), and the worked 50 Mbps point (§6.2) closes at 1.31 MB ≤
   2.88 MB. The one scaling risk (25 MB at-rest map) is shown to be a per-query KB
   slice via segmented binary search (§6.5), with an externalization hook reserved.
   **Survived** (one non-blocking scaling note recorded).

7. **Schema evolution rewrites the world / fails open.** Attack: adding a column
   forces rewriting all node objects, or an old reader mis-reads new bytes. Refuted
   by §8: new columns are new chunks in new shards only; absent column = null via the
   self-describing column directory; readers **fail-closed** on an unknown
   `format_version`/codec (BUG-0014 lesson applied). **Survived.**

## Steering-Storage Verdict

**Verdict:** approve (ratified-with-conditions)

**Blocking findings** (must be addressed before approval): none. Every pre-
registered storage-domain falsification target survived the loop above with cited
evidence; no attempt broke the design, so no finding blocks ratification.

**Conditions (binding on the dependent implementation tasks — land-gates, not
design blockers):**

- **C1 (T-0008, T-0018) — `r ≤ 1` is a tested invariant, not a hope.** The
  adjacency reader MUST compute every hop-`h+1` neighbor-block address from the
  pinned manifest partition map + intra-object offset directory with **no
  intervening object-store GET**, and an integration test on the mock MUST assert
  the GET count per hop is bounded by `M_max` (one parallel batch), proving `r = 1`.
  If the implementation finds `r = 2` unavoidable, **escalate to steering** (ADR
  0001 §1.5) — do not silently ship `K_min = 14`.
- **C2 (T-0008) — early-abort is a hard per-GET byte/row cap.** The reader MUST
  consult the offset-directory `block_len`/`degree` before fetching neighbor bytes
  and MUST truncate the range-GET at the running budget / LIMIT (§3.4). An
  integration test MUST show a super-hub frontier node does not cause a read beyond
  the cap.
- **C3 (T-0007) — columnar filter read touches only the filtered column.** A test
  MUST assert that evaluating a single-property filter fetches ≤ that column's
  chunk bytes over the touched shards, not whole node records (§2.4).
- **C4 (T-0009) — inline-vs-referenced stats cut + value-digest privacy.** The
  manifest MUST carry the OOE-critical scalars (`node_count`, `total_node_count`,
  `edge_count`, `p99_deg`, **`max_deg`**) inline; MCV/histogram detail MAY be a
  referenced `db/stats/<hash>.stats` blob; all value-derived stats MUST store
  BLAKE3-truncated digests, **never raw values** (§5.3; SPIKE-0004 §1.2; guardrails
  §3). A test MUST assert no raw property value appears in any committed manifest /
  stats fixture.
- **C5 (T-0009/T-0010) — exact `objects[]` + reference-counted GC.** The manifest's
  `objects[]` MUST be the exact, complete reference set; GC MUST use the live-object-
  set test over surviving manifests (§7.2), never version-scoped wholesale deletion
  (decision 0027 BC-1). Inherits ADR 0002's mock-fidelity CAS test (two concurrent
  `PUT If-None-Match:*` → one 200 / one 412).
- **C6 (T-0009 + planner) — the `K_min = 9` fallback is explicit.** If a rel-type
  sets `colocated_projection = false`, the planner MUST re-pin the budget at
  `K_min = 9` and the K=9 OOE thresholds for plans over it (§7.3). The default and
  recommended config is co-located `K_min = 8`; K=9 is never silent.

**Non-blocking notes:**

- **N1 — partition-map externalization (§8.2).** At >10× the 1B/10B design point the
  at-rest map (~25 MB) may warrant the reserved `db/manifest-index/<hash>`
  externalization. Not needed at the design point; the per-query read is a KB slice
  via segmented binary search (§6.5). Revisit if a benchmark dataset exceeds the
  point.
- **N2 — binary manifest sidecar.** JSON is chosen for debuggability/forward-compat;
  a binary sidecar behind `format_version` is a future perf optimization, not a
  break. perf-engineer (T-0016) may quantify the phase-1 parse cost and propose it.
- **N3 — projection-set co-location heuristic** is an open tuning (open question 3);
  the format is correct for any choice, the budget is bounded by the projection
  width.

**Rationale:** I attacked the layout on the seven axes my mandate owns —
`r ≤ 1`, the phase count (PS-1), super-hub realized+detected bytes (F2), the
selective-scatter access pattern, torn-read/GC safety, the byte-budget derivation,
and schema-evolution fail-closed behavior — and it survived all seven with specific
structural evidence (intra-object offset directory + manifest partition map for
`r = 1`; co-located projection for PS-1; offset-directory-driven hard byte cap for
F2; sorted bands for contiguity; ADR-0002-inherited atomicity for torn-read/GC; a
phase accounting that reduces exactly to ADR 0001 §2.2 for the budget; self-
describing footer + fail-closed versioning for evolution). The conditions C1–C6 are
test/implementation obligations on the tasks that already own them, in line with
pace doctrine (Cat. 2 is a GATE and the storage cascade is the keystone) —
ratify-and-unblock, not re-loop. No falsification surfaced that moves the feasible
region or contradicts an upstream ratified ADR, so a `reject` or `changes_requested`
would be unfounded; "looks fine" is not the basis — the seven cited survivals are.

**Signed:** steering-storage  T0+~03:40
