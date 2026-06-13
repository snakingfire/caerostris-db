# ADR 0005 — Pluggable secondary-index interface

## Status

`proposed`

## Date / T+ marker

T+3:05 (2026-06-13T21:30:00Z)

## Context

Cat. 5 (pluggable secondary indices, weight 7) requires an index trait/interface
designed so a B-tree (T-0023), and later range / full-text / spatial / composite
indices (T-0025 proves the second type), can be added **without rewriting core
planner or storage logic**. The interface is also load-bearing for the latency
selectivity-envelope theorem (Cat. 3): a selective index anchors an otherwise
unanchored multi-hop `MATCH` to a small seed set, keeping bytes-read inside
`B_max` (`docs/adr/0001-latency-selectivity-envelope.md`).

This ADR records the **interface contract only** (board item T-0022). Concrete,
object-store-persisted implementations come later and are gated on the storage
format spec (SPIKE-0003); the trait is deliberately designed against the logical
data model (T-0006, `done`) so it can land first and prevent B-tree specifics
from leaking into the contract.

Hard constraints shaping the design:

- **No B-tree assumptions in the contract.** Cat. 5 = 100 explicitly needs a
  second index type against the same trait; an interface that bakes in ordering
  would force every future index into a B-tree shape.
- **The planner must consult indices without knowing the concrete type.** The
  planner picks indices *by selectivity* (Cat. 5 anchor "planner picks indices by
  selectivity"); it must hold heterogeneous indices behind one type-erased
  surface.
- **`PropertyValue` is neither `Ord` nor `Eq`** (`src/model/value.rs`): it holds
  `f64` (only `PartialOrd`) and openCypher distinguishes value equality from
  structural identity. An ordered index nonetheless needs a *total* key order.
- **Object storage round-trips are the budget** (Decision 0009): the planner's
  selectivity estimate must be computable without extra object-store reads. This
  ADR keeps selectivity a pure function of index-resident counts; the
  statistics-maintenance contract (SPIKE-0004) is orthogonal and not depended on
  here.

## Decision

We will define a **two-layer index interface** in `src/index/`:

1. A generic, fully-typed trait **`SecondaryIndex`** that every index type
   implements. Its associated `Key` and `Value` types are bounded by `Clone`
   only — **not** `Ord` — so non-ordered key shapes (full-text tokens, spatial
   keys) fit. It exposes `insert`, `delete`, point `lookup`, and a **fallible**
   `range_scan`, plus an `entry_count` for selectivity and a `capabilities()`
   descriptor (`IndexCapabilities`) that *advertises* whether the index supports
   ordered range/prefix queries. An index that does not order keys returns
   `IndexError::RangeUnsupported` from `range_scan` rather than pretending. Range
   bounds are passed as a concrete `KeyRange<Key>` struct (not a generic
   `RangeBounds` parameter) so the method stays non-generic and the trait stays
   object-safe.

2. An object-safe, type-erased **`PropertyIndex`** planner facade specialised to
   the graph case (`PropertyValue` keys → `NodeId` values). The planner consults
   indices through this trait alone: `selectivity(&IndexQuery) -> Selectivity`
   and `probe(&IndexQuery) -> Result<Vec<NodeId>, IndexError>`, plus
   `supports_range()`. A **blanket impl** bridges every
   `SecondaryIndex<Key = OrderedKey, Value = NodeId>` into a `PropertyIndex`, so a
   new index type gains the planner facade for free and the planner never names a
   concrete type.

To give `PropertyValue` a total order without weakening its model semantics, we
will add an **`OrderedKey`** newtype that implements `Ord`/`Eq` by delegating to
`PropertyValue::cypher_order` (the openCypher orderability relation, total over
all values and types). `OrderedKey` is the key type ordered property indices use;
the planner-facing `IndexQuery` speaks plain `PropertyValue` and wraps internally.

An in-memory `InMemoryIndex<K: Ord, V>` reference implementation lands with the
trait to exercise the interface in unit tests; it is explicitly **not** the
object-store B-tree (T-0023). A second, unordered equality-only index type is
sketched in the tests to prove the trait carries no B-tree assumptions.

## Alternatives considered

### Alternative A — Single object-safe `dyn SecondaryIndex` keyed on `PropertyValue`, no generic trait

**Description:** One trait, concrete `PropertyValue` keys and `NodeId` values, no
associated types; the planner holds `&dyn SecondaryIndex`.

**Why considered:** Simplest possible surface; no two-layer indirection; the
planner facade and the index trait would be one thing.

**Why rejected:** It bakes the graph case into the contract — an edge index, a
composite-key index, or a non-`NodeId`-valued index could not implement it
without a parallel trait, which is exactly the "core rewrite to add an index
type" Cat. 5 forbids. It also cannot key on `PropertyValue` directly
(`PropertyValue: !Ord`), so it would still need `OrderedKey` *and* lose
genericity.

### Alternative B — `Key: Ord` bound on the trait, with a `RangeBounds` generic method

**Description:** Require `type Key: Ord` and make `range_scan<R: RangeBounds<Key>>`
generic over the range type.

**Why considered:** `Ord` makes range scans trivially well-defined; `RangeBounds`
is idiomatic Rust.

**Why rejected:** Two problems. (1) `Key: Ord` forces *every* index — including
hash and future spatial/full-text indices whose keys have no natural total order
— to fabricate an order, leaking the B-tree assumption the task explicitly bans.
(2) A generic `range_scan` method makes the trait **not object-safe**, so the
planner could not hold `&dyn` indices in a heterogeneous catalog. We instead
moved ordering into the per-implementation key bound (`InMemoryIndex<K: Ord>`)
and use a concrete `KeyRange` struct.

### Alternative C — Make `PropertyValue` itself derive `Ord`/`Eq`

**Description:** Add `Eq`/`Ord` derives (or hand-impls) directly on
`PropertyValue` so it can be an index key without a wrapper.

**Why considered:** Removes the `OrderedKey` newtype entirely.

**Why rejected:** It is semantically wrong and would corrupt the model: `f64` is
not `Ord` (NaN), and openCypher's `=` operator (`cypher_equal`, ternary) is *not*
the structural identity `PartialEq` already implements. Forcing `Ord`/`Eq` onto
`PropertyValue` would conflate orderability with value equality and risk silent
TCK regressions. The newtype localises the "total order for indexing" concern to
the index layer where it belongs.

## Consequences

### Positive

- Advances Cat. 5 toward 100: pluggable trait defined; selectivity-aware
  planner facade in place; a second index type proven against the same trait
  (the extensibility criterion) — all without core rewrites.
- Unblocks T-0023 (object-store B-tree implements `SecondaryIndex`) and T-0025
  (second concrete index type) on a stable contract.
- Object-safe planner facade lets EPIC-002 hold a heterogeneous index catalog
  behind `&dyn PropertyIndex` and select by selectivity — the mechanism that
  anchors the Cat. 3 latency envelope.
- `OrderedKey` reuses the already-tested `cypher_order` total order, so index key
  ordering and openCypher `ORDER BY` cannot drift apart.

### Negative / trade-offs

- Two layers (generic trait + type-erased facade) add indirection; a reader must
  understand both. Mitigated by the blanket impl (one bridge, written once) and
  module docs.
- `KeyRange` re-implements a slice of `RangeBounds` to stay object-safe — minor
  duplication of `std`'s range vocabulary.
- `Selectivity` here is computed from index-resident counts only. A production
  estimator that must avoid scanning the whole index for a range count (large
  cold indices) will need the SPIKE-0004 maintained-statistics contract; this ADR
  does not solve that and does not need to (interface-only task).

### Open questions

- Persisted-index lifecycle (how an index object is versioned and swapped
  atomically with the data manifest) — deferred to T-0023 + SPIKE-0003.
- Composite / multi-property keys: the `Key` associated type can be a tuple, but
  the `PropertyValue`-keyed facade covers single-property indices only for now. A
  later ADR extends the facade if composite indices are needed.
- Cost-based selectivity thresholds (when is an index cheaper than a scan, in
  bytes) — owned by the planner (EPIC-002) / SPIKE-0004, not this interface.

## Rubric impact

| Cat. | Name | Impact |
|------|------|--------|
| 5 | Secondary indices | Defines the pluggable trait + selectivity-aware planner facade + a second index type sketch — the structural prerequisites for Cat. 5 = 100. |
| 3 | Latency envelope | Provides the `PropertyIndex::selectivity` surface the planner uses to anchor unanchored matches inside `B_max`. |
| 4 | openCypher (planner) | The type-erased facade is what the planner consults to turn `WHERE n.prop = x` into an index probe. |

## Sign-off

### Adversarial review record

_(no rounds yet)_

### Steering ratification

_(pending adversarial review)_
