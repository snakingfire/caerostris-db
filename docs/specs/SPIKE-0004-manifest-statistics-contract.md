# SPIKE-0004 — Manifest Statistics Contract for Out-of-Envelope Detection

> **Type:** design/research spec (no code). Decision artifact for steering ratification.
> **Owner:** researcher (this spike). **Joint steering item:**
> `steering-query-cypher` (primary, Cat. 4/5 planner+index) + `steering-storage`
> (Cat. 2 manifest layout) + `steering-perf-sla` (Cat. 3 envelope/detection).
> **Status:** proposed — awaiting ≥3-of-5 cross-cutting quorum (sign-off request:
> `.project/decisions/0030-spike-0004-statistics-contract-signoff-request.md`).
> **Rubric:** Cat. 3 (latency envelope, GATE, w14), Cat. 4 (planner, GATE, w12),
> Cat. 5 (index selectivity, w7); touches Cat. 2 (manifest, GATE, w12).

## Research question (restated)

> *What exact set of graph statistics must the storage layer maintain and publish
> in the manifest so the query planner can soundly classify a 6-hop unanchored
> match as in- or out-of-envelope in `O(plan-size)` before any object-store
> access — using a **tail/worst-case** (not mean) fan-out bound — and what does the
> planner do when a needed statistic is missing or stale?*

This spike does **not** re-open the envelope algebra (ADR 0001, ratified). It pins
the **estimator inputs** that ADR 0001 §4.1 reads from "manifest statistics
(SPIKE-0004)" and that ADR 0001 finding **F2** / decision 0015 require to include a
per-rel-type **max** out-degree, not only a p99.

## Why this exists (the gap)

ADR 0001's out-of-envelope (OOE) detection (Part 4) computes, before any I/O:

```
est_N_seed  = N_nodes(label) × selectivity(property, value)
est_F_tail  = per-rel-type p99 out-degree
est_B_query = bytes_manifest + est_N_seed·bytes_node + 6·M_max·est_F_tail·bytes_edge_row
```

Every one of `N_nodes(label)`, `selectivity(property,value)`, and the per-rel-type
degree summary is sourced from "the manifest" — but **no artifact named the contract**:
the exact statistic set, its on-object encoding, how it is maintained on commit
snapshot-consistently, and the missing/stale rule. Without that contract:

- The planner cannot size the seed set ⇒ it guesses selectivity.
- ADR 0001 **F2 / decision 0015** is unbound: a p99 fan-out admits the ~1 % of
  super-hub nodes above it; one un-truncated adjacency list over a 10⁵–10⁸-degree
  node is 6.4 MB–6.4 GB and alone busts B_max (2.88 MB at 50 Mbps). An estimator
  sized from p99 is **optimistic in the dangerous direction** — the exact silent
  SLA miss the invariant forbids (commander's intent L62, L101).
- An "optimistic guess" on a freshly-ingested graph with no stats yet is a silent
  accept of a query that will blow the budget.

This spec closes that gap.

---

## Part 1 — The Statistics Set (the contract)

The manifest publishes a **statistics block** `stats` covering three families.
All counts are **exact as of the committed version** (maintained incrementally on
commit; see Part 2); all distribution summaries are **bounded estimates** with an
explicit freshness marker.

### 1.1 Per-label node statistics

For each node label `ℓ` present in the version:

| Field | Type | Meaning | Used by |
|-------|------|---------|---------|
| `node_count[ℓ]` | `u64` (exact) | Number of nodes carrying label `ℓ`. | `est_N_seed` upper bound; Cat. 6 fast `count`. |
| `total_node_count` | `u64` (exact) | Total nodes in the graph (for `s = N_seed/N_total`). | `s_max` check (ADR §2.2). |

`node_count` doubles as a **fast-aggregate source** (Cat. 6): `MATCH (n:ℓ) RETURN
count(n)` is answered from the manifest with zero data GETs.

### 1.2 Per-property selectivity statistics

For each `(label ℓ, property p)` the planner may filter on, a **selectivity
estimator** sized to answer `selectivity(p, value) = P[n.p = value | n:ℓ]`:

| Field | Type | Meaning |
|-------|------|---------|
| `ndv[ℓ,p]` | `u64` (estimate) | Number of distinct values of `p` over `ℓ` (HyperLogLog cardinality estimate; see Part 4). |
| `null_frac[ℓ,p]` | `f32` | Fraction of `ℓ`-nodes where `p` is absent/null. |
| `most_common[ℓ,p]` | `Vec<(value_digest, u64 count)>` | The top-`H` most-common values (digest + exact count) — the **MCV list**, default `H = 32`. Captures skew (a hot value like a country code is not uniform). |
| `histogram[ℓ,p]` (text/ordered) | `Vec<(bound_digest, u64 cumulative)>` | Equi-depth quantile boundaries over the **non-MCV remainder**, default `Q = 64` buckets. Supports range and prefix selectivity for the B-tree index (Cat. 5). |

**Selectivity derivation (planner side, O(1) per predicate):**

- **Equality `n.p = v`:** if `digest(v) ∈ most_common` → `count / node_count[ℓ]`
  (exact-ish). Else (non-MCV) → `(1 − Σ mcv_counts/node_count[ℓ] − null_frac) /
  max(1, ndv − |most_common|)` — the standard uniform-remainder estimate.
- **Range / prefix `n.p < v` / `STARTS WITH 'x'`:** interpolate over `histogram`
  cumulative counts. (Prefix is a range `[x, x+1)` over the collation.)
- **Unknown property / no estimator:** selectivity is **unknown** ⇒ planner uses
  the conservative upper bound `selectivity = 1` (Part 3, OOE-5).

> **Value digests, not raw values.** The manifest stores a fixed-width
> collision-resistant digest of each MCV/boundary value (e.g. the first 8 bytes of
> a BLAKE3 hash) plus, for ordered histograms, the order-preserving truncated key
> needed for interpolation — **never the raw property value**. This (a) bounds the
> stats-block size independent of value length, (b) keeps the open-source repo and
> any committed fixtures free of user data by construction (guardrails §3), and
> (c) is sufficient because the planner only needs equality-match against a query
> literal's digest and ordered comparison for ranges.

### 1.3 Per-relationship-type degree statistics

For each relationship type `τ` and direction `d ∈ {out, in}`:

| Field | Type | Meaning | Used by |
|-------|------|---------|---------|
| `edge_count[τ]` | `u64` (exact) | Total edges of type `τ`. | Cat. 6 `count`; mean-degree denominator. |
| `mean_deg[τ,d]` | `f32` | `edge_count[τ] / count(source nodes)`. | Diagnostics only — **never** the byte estimator. |
| `p99_deg[τ,d]` | `u32` (estimate) | 99th-percentile out/in-degree. `F_tail` in ADR §2.2 typical-case sizing. | `est_F_tail` (typical-frontier byte estimate). |
| **`max_deg[τ,d]`** | `u32` (exact or upper-bounded) | **Maximum** out/in-degree of any node over type `τ`. **The super-hub safety term.** | ADR F2 / decision 0015 byte **safety** bound; OOE-2 super-hub reject. |
| `degree_hist[τ,d]` (optional) | `Vec<(deg_bound, u64 node_count)>` | Coarse degree histogram (log-spaced buckets) — refines frontier sizing when present. | Optional tightening of `est_F_tail`. |

**`max_deg` is mandatory, per decision 0015 / ADR 0001 F2.** It is the only term
that makes the super-hub case detectable at plan time. `p99_deg` alone is
**insufficient and is forbidden as the byte safety bound** — it admits the super-hub.

`edge_count[τ]` doubles as a fast-aggregate source (Cat. 6).

### 1.4 Block-level metadata (freshness & soundness)

| Field | Type | Meaning |
|-------|------|---------|
| `stats_version` | `u32` | Schema version of the statistics block (forward-compat / evolution). |
| `as_of_version` | `u64` | The committed manifest version these stats describe (== this manifest's `V`). |
| `freshness` | enum `{exact, estimated, stale, absent}` per family | Per-statistic-family marker. Drives the missing/stale rule (Part 3). |
| `estimator_params` | record | `{hll_precision, mcv_H, hist_Q, sketch_seed}` — so a reader can reason about error bounds and a re-`ANALYZE` is reproducible. |

---

## Part 2 — Where statistics live & how they are maintained (with `steering-storage`)

### 2.1 Home: the manifest (snapshot-consistent, zero extra round-trip)

The statistics block lives **inside the manifest object**
`db/manifest/<V>.json` (ADR 0002 §1, layout table), as a top-level `stats` field.

**Rationale (binding):**

- ADR 0001 §4.1/§4.3 require the planner to read stats **in `O(plan-size)` before
  any object-store access** and **snapshot-consistently with the pinned version**.
  The manifest is *already* the one object a reader resolves on open (ADR 0002 §4:
  reader resolves `max(LIST db/manifest/)`, then reads `db/manifest/<V>.json`).
  Co-locating stats in the manifest means **the version pin and the statistics are
  the same read** — no extra GET, no skew between "which version" and "whose stats".
- Manifests are **immutable, created exactly once** (ADR 0002 §1). So the stats a
  planner reads for version `V` are exactly the stats the committer of `V` wrote —
  consistent by construction, for every attach mode (including master-less and
  embedded read-only, which have no live writer to ask).

**Decision — separate `stats` object vs. inline.** For a 1B/10B graph the stats
block is **bounded and small** (Part 5 size analysis: ~tens of KB to low MB,
dominated by per-(label,property) MCV+histogram entries). Two encodings:

- **(A) Inline in the manifest JSON.** Simplest; one GET resolves version + full
  stats. Risk: if the manifest grows large (many labels × properties × H+Q
  entries), the manifest GET itself grows — but it stays well under B_max and is a
  single sequential read in phase 1, which the cost model already budgets.
- **(B) Manifest header + referenced `stats` blob(s).** The manifest carries the
  *small* always-needed scalars (`node_count`, `edge_count`, `max_deg`, `p99_deg`
  — the OOE-decision-critical terms) inline, and references larger per-property
  MCV/histogram blobs (`db/stats/<content-hash>.stats`, content-addressed like any
  data object, GC-ed via the same manifest-reference-set rule) that the planner
  fetches **only if** the query filters on that property.

**Recommendation: (B) hybrid, with a hard inline floor.** The OOE-critical scalars
(`total_node_count`, per-label `node_count`, per-rel-type `p99_deg` **and**
`max_deg`, `edge_count`) are **always inline** so that OOE-1/2/3/4 and the
super-hub safety check (F2) need **zero** extra reads — they run purely on the
phase-1 manifest. The bulkier per-property selectivity detail (MCV lists,
histograms) may be a referenced content-addressed blob, fetched lazily during
planning **only** for properties the query actually filters on. Because that fetch
is a planning-time read, it counts as an O(plan-size) statistics-lookup, not a
data-plane GET, and it is bounded (Part 5). `steering-storage` owns the final
inline-vs-referenced cut; the **invariant** they must preserve is: *the terms OOE
detection needs to reject a super-hub or a non-selective filter are reachable
without a data-plane round-trip beyond the manifest itself.*

> This refines, and is consistent with, ADR 0002 §1's "the manifest … carries the
> snapshot-consistent statistics" and T-0009's "statistics readable from the pinned
> manifest with no extra round-trip beyond resolving the manifest itself" — the
> referenced-blob fetch in (B) is a *planning-phase* lookup for filtered properties,
> still snapshot-consistent (content-addressed, referenced by the pinned manifest)
> and still off the K-phase data path.

### 2.2 Maintenance on commit (incremental, single-writer)

The single-writer model (R2) makes maintenance tractable: there is exactly one
committer, so statistics are updated **as part of building version V+1's manifest**,
under the writer lease, before the atomic manifest-create (ADR 0002 §2/§3).

**Exact counts (`node_count`, `total_node_count`, `edge_count`) — incremental, exact.**
A commit knows its own delta (nodes/edges added/removed in this transaction). The
new manifest's exact counts = previous manifest's counts + this commit's delta.
`O(distinct labels/rel-types touched)`, not `O(graph)`. Always `freshness = exact`.

**`max_deg` — incremental upper bound, exact on full recompute.**
- On an **incremental** commit, `max_deg[τ,d]` is updated to
  `max(prev_max_deg, max over nodes whose degree changed in this commit)`. A commit
  can only *raise* a node's degree it touches; a node it does not touch cannot
  exceed the previous `max_deg`. So `new_max_deg = max(prev_max_deg, max degree of
  any node modified this commit)` is a sound **upper bound** (and exact unless a
  *deletion* lowered the true max below `prev_max_deg`, in which case the stored
  value is conservatively high — which is **safe** for OOE: it over-rejects, never
  under-rejects). Marked `freshness = exact` (counts) / the degree summary carries
  its own marker.
- A periodic/explicit **`ANALYZE`** (full recompute, see 2.3) tightens `max_deg`
  back to the true maximum and refreshes `p99_deg`, MCV, histograms, NDV.

**`p99_deg`, `ndv`, MCV, histogram — estimated, refreshed on `ANALYZE`.**
These are distribution summaries that cannot be maintained exactly under arbitrary
incremental edits without re-scanning. Policy:
- They are **recomputed on `ANALYZE`** (a full or sampled scan; sampling bound in
  Part 4) and stamped with `as_of_version` = the version they were computed at.
- On an incremental commit that does **not** run `ANALYZE`, they are **carried
  forward** from the parent manifest and their `freshness` is downgraded to
  `estimated` (still usable) or `stale` (see staleness rule, Part 3) based on how
  much the graph has changed since `as_of_version` (a drift counter: rows changed /
  total rows since last `ANALYZE`).
- A freshly-ingested label/property/rel-type with **no** computed summary yet is
  marked `freshness = absent` ⇒ the planner treats its selectivity/fan-out as
  unknown (Part 3 conservative rule).

**Crash/partial-write safety.** Because stats live in (or are referenced by) the
immutable manifest and the manifest-create is the atomic commit point (ADR 0002
§2), statistics become visible **iff** the commit succeeds. A crashed commit
leaves no manifest ⇒ no stats update is ever partially visible. This inherits the
commit protocol's atomicity directly; no separate stats-durability mechanism is
needed.

### 2.3 `ANALYZE` / `REFRESH STATISTICS`

A maintenance operation (manual `ANALYZE`, or writer-triggered when the drift
counter crosses a threshold) that recomputes the estimated summaries (p99/max
degree to exact, NDV, MCV, histograms) over the current version and commits a new
manifest whose `stats` are `freshness = exact/estimated` and `as_of_version =`
the new version. It is an ordinary single-writer commit (no new protocol). The
sampling/precision knobs are in `estimator_params`.

---

## Part 3 — The estimator (tail/worst-case) and the missing/stale rule

### 3.1 Two distinct degree terms — typical vs. safety

ADR 0001 finding F2 / decision 0015 is discharged by giving the estimator **two**
degree terms with **different jobs**:

| Term | Statistic | Role | Direction of error if wrong |
|------|-----------|------|------------------------------|
| **Typical fan-out** | `p99_deg[τ,out]` | Sizes the *expected* per-hop byte cost `6·M_max·F_tail·bytes_edge_row` for the in-envelope acceptance estimate. | Tuning; not safety-critical because the safety term below dominates rejection. |
| **Safety fan-out** | `max_deg[τ,out]` (or the storage hard per-GET byte/row cap) | Detects the **super-hub**: if any node the frontier can reach has `max_deg` whose single adjacency list exceeds the per-GET byte cap / busts B_max, the query is **conservatively rejected**. | Must be **conservative (over-reject)** — never optimistic. |

The byte **acceptance** estimate uses `p99_deg` (typical); the byte **safety**
gate uses `max_deg`. A query passes only if it clears **both**: estimated bytes ≤
B_max under the typical term **and** no reachable rel-type's `max_deg` adjacency
list can individually bust the per-GET byte cap. This is exactly the F2 split:
realized over-read is prevented by storage early-abort (SPIKE-0003 / SPIKE-0008
F1, restated as a hard per-GET byte/row cap), and *detection* of the super-hub is
done here from `max_deg`.

### 3.2 Worked super-hub example (the case the planner MUST classify OOE)

Design point: 50 Mbps, `B_max = 2.88 MB`, `bytes_edge_row = 64 B`, `M_max = 8`.
Per-GET adjacency byte cap (storage early-abort) `C_get` — set to `B_max` for the
worst single GET; realistically the executor enforces the *running* B_max budget.

Rel-type `FOLLOWS`, with maintained stats:
```
p99_deg[FOLLOWS,out] = 120          (typical influencer)
max_deg[FOLLOWS,out] = 4.0e7        (a celebrity super-hub: 40M followers)
bytes for one max-degree adjacency list = 4.0e7 × 64 B = 2.56 GB
```

- **Typical (p99) acceptance estimate** (for a tiny seed set, say `N_seed = 5000`):
  `est_B_query ≈ 4096 + 5000·256 + 6·8·120·64 = 4096 + 1,280,000 + 368,640
  ≈ 1.65 MB ≤ 2.88 MB` → looks in-envelope on p99 alone. **This is the trap:** if
  the frontier can route through the celebrity node, the realized adjacency read
  for that one node is 2.56 GB — 890× B_max.
- **Safety gate (max_deg):** `max_deg[FOLLOWS,out] × bytes_edge_row = 2.56 GB ≫
  C_get (= B_max = 2.88 MB)`. The single super-hub adjacency list alone exceeds the
  per-GET byte cap ⇒ the planner classifies the query **out-of-envelope** (OOE-2
  super-hub branch) and rejects/warns, *even though the p99 estimate passed*.

This is the precise failure decision 0015 demanded be caught at plan time, and it
is caught **only** because `max_deg` is in the contract.

> **Note on whether the seed can reach the hub.** A sound planner does not assume
> the seed cannot reach the hub: across 6 unanchored hops over a rel-type with a
> 40M-degree node, the probability the frontier touches it is non-trivial, and the
> invariant forbids an optimistic assumption. The conservative rule is: **if a
> rel-type in the plan has a `max_deg` whose adjacency list busts the per-GET byte
> cap, and the executor cannot prove the hub is unreachable, classify OOE.** The
> LIMIT-driven early-abort still protects *realized* latency if such a query is run
> under an explicit `WARN` override (ADR §4.4) — but the default is reject.

### 3.3 Missing / stale / absent statistics → conservative, never optimistic

This is the single most important safety rule and is **written down here** as the
binding doctrine (consistent with ADR §4.1, OOE-5, decision 0009):

> **The planner never makes an optimistic assumption from a statistic it does not
> trust. Absent/stale/unknown ⇒ assume the worst case ⇒ default to reject/warn.**

Concretely, per statistic family:

| Statistic state | Planner assumption | Effect on OOE |
|-----------------|--------------------|---------------|
| **selectivity `absent`/`unknown`** (no MCV/hist/NDV for the filtered property) | `selectivity = 1` (filter matches **all** `ℓ`-nodes) ⇒ `est_N_seed = node_count[ℓ]` | If `est_N_seed > s_max·N_total` (almost always for a non-trivial label) ⇒ **OOE-5 reject/warn**. |
| **`max_deg` `absent`** for a rel-type in the plan | `max_deg = ∞` (assume a super-hub exists) | Super-hub safety gate fails ⇒ **OOE-2 reject** unless `ALLOW_MISSING_STATS`. |
| **`p99_deg` `absent`** | fall back to `max_deg` (or ∞ if also absent) for the typical term too | Byte estimate inflates ⇒ likely OOE. |
| **stats `stale`** (drift since `as_of_version` over threshold `θ`, default 20 % of rows changed) | Treat estimated terms as `estimated` but **degrade `max_deg` to its incremental upper bound** (which is still sound — 2.2) and **selectivity to unknown if the property's MCV/hist predates the drift window** | Conservative; may over-reject until `ANALYZE`. |
| **stats `exact`/`estimated`, fresh** | Use as published. | Normal acceptance path. |

**Override surface (never silent):** the only way to run a query the planner would
reject for missing/stale stats is the explicit, per-query/session hints already in
ADR §4.4 — `SET envelope_check = WARN`, `ALLOW_MISSING_STATS`. When overridden the
engine **executes but emits a structured warning that the SLA is not guaranteed**.
It never silently accepts.

**Freshly-ingested data, no stats yet (the named case in the acceptance criteria):**
After a bulk ingest with no `ANALYZE`, selectivity for new properties is `absent`
and degree summaries are at their incremental upper bound. Per the table above the
planner **conservatively rejects/warns** non-trivial 6-hop queries and the warning
message tells the operator to run `ANALYZE` / `REFRESH STATISTICS`. This is the
correct, safe behavior: it is better to reject a query that *might* be in-envelope
than to accept one that *might* blow the SLA — the invariant is asymmetric.

---

## Part 4 — Estimator choices (license-checked) and error bounds

The statistics themselves are computed by the writer/`ANALYZE` path. The
estimation *algorithms* are standard and implementable in-tree; where a crate
helps, it must be license-clean. We recommend **implementing the sketches in-tree**
(they are small, well-specified, and keep the dependency surface minimal), with
named permissive crates as optional accelerators.

### 4.1 NDV (distinct values) — HyperLogLog

- **Algorithm:** HyperLogLog / HLL++ (Flajolet et al. 2007; Heule et al. 2013).
  Standard error ≈ `1.04/√(2^precision)`; at precision 14 (16 KB) ≈ 0.81 %. Bounded,
  mergeable across shards (matches columnar layout).
- **In-tree** is the recommendation (a few hundred lines). Optional crate:

```
Name: hyperloglogplus
License: Apache-2.0 OR MIT
Compatible with caerostris-db (permissive): yes
Source: https://crates.io/crates/hyperloglogplus
```
```
Name: probabilistic-collections   (HLL, count-min, etc.)
License: Apache-2.0 OR MIT
Compatible with caerostris-db (permissive): yes
Source: https://crates.io/crates/probabilistic-collections
```

### 4.2 MCV (most-common values) — count-min sketch or sampled top-K

- **Algorithm:** Space-Saving / count-min sketch for heavy hitters, or an exact
  top-`H` over a scan (single-writer, so a scan during `ANALYZE` is fine for the
  one-time cost). MCV captures skew so a hot value is not assumed uniform.
- Same `probabilistic-collections` crate (Apache-2.0 OR MIT) provides count-min if
  desired; in-tree top-K over a scan is simplest.

### 4.3 Histograms / quantiles — equi-depth or t-digest

- **Algorithm:** equi-depth quantiles from a sort/sample during `ANALYZE`, or a
  streaming t-digest (Dunning 2019) if incremental quantiles are wanted later.

```
Name: tdigest
License: Apache-2.0 OR MIT
Compatible with caerostris-db (permissive): yes
Source: https://crates.io/crates/tdigest
```

### 4.4 Degree p99 / max — exact on `ANALYZE`, incremental bound between

- `max_deg` and the degree histogram are exact on a full `ANALYZE` scan (the
  writer already touches adjacency lists). `p99_deg` from the degree histogram.
  Between `ANALYZE`s, `max_deg` is the incremental upper bound (Part 2.2).
- The digest for value privacy uses **BLAKE3** (already a likely repo dependency
  for content-addressing in ADR 0002):

```
Name: blake3
License: CC0-1.0 OR Apache-2.0 OR Apache-2.0-with-LLVM-exception
Compatible with caerostris-db (permissive): yes
Source: https://crates.io/crates/blake3
```

**No GPL/AGPL/SSPL/BUSL or distribution-restricted dependency is required or
recommended.** All four named crates are dual permissive (Apache-2.0 OR MIT, or
CC0). Per guardrails §5, the implementer runs `cargo deny check licenses` before
adding any of them; the recommendation is to **prefer in-tree implementations** to
minimize the surface, with these as vetted fallbacks. (Guardrails §5 audit
discharged at recommendation time; final verification at add-time per BUG-0008's
AND/OR SPDX precedence rule — all the above are OR-conjunctions, i.e. permissive.)

---

## Part 5 — Size & overhead analysis (the stats block stays small)

For a graph with `L` labels, `P` filtered properties per label, `R` rel-types,
default `H=32` MCV entries (16 B digest + 8 B count = 24 B each), `Q=64` histogram
buckets (16 B each), HLL precision 14 (16 KB per property NDV — but only the final
estimate, a `u64`, is stored in the manifest; the 16 KB register array lives in the
`ANALYZE` path, not the manifest):

```
per-(label,property) stored: ndv(8) + null_frac(4) + MCV(32×24=768) + hist(64×16=1024) ≈ 1.8 KB
per-label inline scalars:    node_count(8)
per-rel-type inline scalars: edge_count(8) + p99_deg(4) + max_deg(4) + mean_deg(4) ≈ 20 B
```

A schema with `L=50` labels, `P=4` filtered properties each, `R=30` rel-types:
```
inline OOE-critical scalars: 50·8 + 30·20 + 16 ≈ 1.0 KB   (always in the manifest)
referenced selectivity blobs: 50·4·1.8 KB ≈ 360 KB         (fetched lazily per filtered property)
```

The **always-inline** OOE-critical part is ~1 KB — negligible against the
`bytes_manifest ≤ 4 KB` the cost model already reserves (ADR §2.2). The bulky
selectivity detail is referenced and fetched only for filtered properties (one
content-addressed blob GET during planning, bounded at a few KB per property),
keeping phase-1 small and the super-hub/non-selective rejection paths zero-extra-GET.

Write overhead per commit: `O(labels + rel-types touched)` for exact counts +
incremental `max_deg`; full `ANALYZE` is `O(graph)` but amortized (manual/triggered,
not per-commit). This is the trade-off ADR 0001 "Consequences/Negative" already
flagged ("statistics must be maintained on every commit") — bounded here to the
*touched* schema, not the whole graph, for the per-commit path.

---

## Part 6 — Decision summary (what is now binding on implementation)

1. **Statistics set (Part 1):** per-label `node_count` + `total_node_count`
   (exact); per-(label,property) `ndv`/`null_frac`/MCV/`histogram` (selectivity);
   per-rel-type `edge_count`/`mean_deg`/`p99_deg`/**`max_deg`**/optional
   `degree_hist`; block metadata (`stats_version`, `as_of_version`, `freshness`,
   `estimator_params`). **`max_deg` is mandatory** (decision 0015 / ADR F2).
2. **Home (Part 2):** in the immutable manifest. OOE-critical scalars
   (`node_count`, `total_node_count`, `edge_count`, `p99_deg`, `max_deg`) **inline**
   (zero extra GET); bulky per-property selectivity detail may be a referenced
   content-addressed blob fetched lazily during planning for filtered properties
   only. Snapshot-consistent with the pinned version by construction.
3. **Maintenance (Part 2.2):** exact counts and `max_deg` maintained incrementally
   per commit (`O(touched schema)`); p99/NDV/MCV/histogram recomputed on `ANALYZE`,
   carried forward with a downgraded `freshness` between. Atomicity inherited from
   the commit protocol (ADR 0002).
4. **Estimator (Part 3):** two degree terms — `p99_deg` for the typical byte
   estimate, **`max_deg` (or storage per-GET byte cap) for the super-hub safety
   gate**. A query must clear both. Worked super-hub example (3.2).
5. **Missing/stale/absent rule (Part 3.3):** conservative, never optimistic —
   absent selectivity ⇒ `s=1`; absent `max_deg` ⇒ `∞`; stale ⇒ degrade to sound
   bounds; default reject/warn; the only escape is an explicit, warning-emitting
   override. Freshly-ingested data ⇒ reject/warn + "run `ANALYZE`".
6. **License (Part 4):** all recommended sketch crates are dual-permissive
   (Apache-2.0 OR MIT / CC0); in-tree implementation preferred; no copyleft/
   restricted dependency needed.

---

## Risks and open questions

- **R1 — inline-vs-referenced cut is `steering-storage`'s call.** Part 2.1 (B)
  recommends a hybrid with a hard inline floor for OOE-critical scalars. If
  steering prefers fully-inline (A), the manifest grows but the cost model still
  holds (manifest GET is sequential phase 1, bounded). The *invariant* to preserve
  is "super-hub/non-selective rejection needs no data-plane GET." **Confidence: high**
  that the invariant is right; **medium** on the exact cut — defer to steering-storage.
- **R2 — `max_deg` as incremental upper bound can over-reject after large
  deletions** until the next `ANALYZE`. This is the *safe* direction (over-reject,
  never under-reject) and is acceptable; `ANALYZE` tightens it. Named, not hidden.
- **R3 — selectivity for composite/correlated predicates** (`WHERE n.a=1 AND
  n.b=2`) uses independence by default, which can under-estimate selectivity (=
  over-estimate seed size = conservative for OOE). Multi-column stats are a future
  extension (out of scope here; the index trait — EPIC-005 — can carry composite
  stats later). Conservative direction, so safe.
- **R4 — drift threshold `θ` (default 20 %) and `ANALYZE` trigger cadence** are
  tuning parameters, not correctness; benchmarks (T-0016) and the sim (T-0014) can
  refine. Wrong `θ` only changes *when* we over-reject vs. trust estimates, never
  whether we under-reject.
- **R5 — whether the seed frontier can reach a known super-hub** is not decided
  statically with precision; we take the conservative stance (3.2 note). A future
  refinement could use reachability stats to relax this, but that is an
  optimization, not a correctness requirement, and is out of scope.
- **Confidence overall: high.** The contract discharges every acceptance-criterion
  bullet and every ADR 0001 F2 / decision 0015 condition. The one genuinely open
  knob (R1) is a storage-layout refinement that does not move the feasible region.

---

## Cross-references (to be threaded by this spike's commit)

- **ADR 0001 §4.1 / §2.2 / F2:** this spec supplies the named statistics those
  sections read; `max_deg` discharges F2 / decision 0015.
- **ADR 0002 §1 / §2:** statistics home (immutable manifest) + atomicity inheritance.
  (The ADR's "SPIKE-0009" reference for manifest stats is corrected to **SPIKE-0004**.)
- **SPIKE-0003 (storage format):** the inline-vs-referenced cut, content-addressed
  `db/stats/<hash>.stats` blob layout + GC, and the early-abort per-GET byte/row cap
  (F2's realized-latency protection) are folded into the storage spec.
- **EPIC-002 / T-0015 (planner OOE detection):** consumes this contract; F1
  (α-corrected thresholds) + F2 (max-degree safety gate) are the binding conditions.
- **EPIC-005 (secondary indices):** the B-tree index's selectivity feeds, and is
  fed by, the per-property histogram/MCV; composite-stats extension noted (R3).
- **EPIC-001 / T-0009 (manifest implementation):** implements this statistics block.
- **Cat. 6 (fast aggregates):** `node_count`/`edge_count` are manifest-resident
  exact counts answering `count(...)` with zero data GETs.

## Steering sign-off (cross-cutting, ≥3-of-5 quorum required)

Per `steering-committee.md` the cross-cutting quorum is majority (≥3 of 5).
Owners for this artifact: **`steering-query-cypher`** (primary — planner/selectivity,
Cat. 4/5), **`steering-storage`** (manifest layout, Cat. 2), **`steering-perf-sla`**
(envelope/detection, Cat. 3). Sign-off request:
`.project/decisions/0030-spike-0004-statistics-contract-signoff-request.md`.
Until ≥3-of-5 ratify, this spec is `proposed`; T-0009 and T-0015 stay `backlog`
on their `SPIKE-0004` dependency.
