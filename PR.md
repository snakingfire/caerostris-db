# PR: T-0022 — Define pluggable secondary-index trait (insert/delete/lookup/range)

## Board item

[.project/board/tasks/T-0022-pluggable-index-trait.md](.project/board/tasks/T-0022-pluggable-index-trait.md)

Branch: `work/T-0022-pluggable-index-trait` (based on the latest `main`, `c3cc51a`).

## Rubric refs

Cat. 5 (pluggable secondary indices). Supports Cat. 3 (latency envelope — index
selectivity anchors unanchored matches) and Cat. 4 (planner consults indices).

## Acceptance criteria (from board item)

- [x] `SecondaryIndex` trait defined: `insert`, `delete`, point `lookup`, and `range_scan`; associated key/value types parameterised so non-B-tree indices fit.
- [x] A planner-facing query API consults the trait by selectivity without knowing the concrete index type.
- [x] An in-memory reference implementation of the trait exists for unit-testing the interface (not the object-store B-tree yet).
- [x] The trait carries no B-tree-specific assumptions (verified by sketching a second index type's signature against it).
- [x] tests added (unit on the in-memory impl); coverage not regressed
- [x] docs / ADR updated with the index-interface decision
- [x] `./format_code.sh` green

## Summary of change

Adds `src/index/` defining the **interface contract** for pluggable secondary
indices (Cat. 5), designed against the logical data model (T-0006). Two layers: a
generic `SecondaryIndex<Key, Value>` trait that every index type implements
(`insert` / `delete` / point `lookup` / fallible `range_scan`, plus `entry_count`
and an `IndexCapabilities` descriptor), and an object-safe, type-erased
`PropertyIndex` planner facade keyed on `PropertyValue` → `NodeId` that the
planner consults **by selectivity** without naming a concrete index type. A
blanket impl bridges any `SecondaryIndex<OrderedKey, NodeId>` into the facade.
Ordering is **advertised** via `capabilities()` rather than assumed, and
`range_scan` returns `IndexError::RangeUnsupported` for indices that cannot order
keys — so the trait carries no B-tree-specific assumptions. The `OrderedKey`
newtype gives `PropertyValue` (deliberately `!Ord`/`!Eq`) a total order by
delegating to the existing `cypher_order` relation; this design need was surfaced
by the TDD RED step. `InMemoryIndex` is a `BTreeMap`-backed reference impl for
exercising the interface (not the object-store B-tree, which is T-0023), and a
second, unordered, `Vec`-backed equality-only index type is sketched in the tests
to prove the trait is not B-tree-shaped (acceptance criterion 4). The decision,
two-layer rationale, and three rejected alternatives are recorded in
`docs/adr/0004-pluggable-index-interface.md` (status: proposed). Object-store
persistence and atomic index/data commit are explicitly out of scope here and
remain gated on SPIKE-0003 / EPIC-004.

## Test evidence

`cargo nextest run` — full suite green:

```
Summary [   3.620s] 156 tests run: 156 passed, 0 skipped
```

Of these, 33 are new `index::tests` covering: insert/delete/lookup (multi-valued,
idempotent insert, key-drop on last delete), range_scan (half-open / from / until
/ explicit bounds / prefix-via-range / numeric order), generic non-text key+value
types, `Selectivity` (fraction, empty-index = least selective / no div-by-zero,
clamping, threshold), the `PropertyIndex` facade (selective→use, unselective→scan
fallback, range probe, boxed `dyn` object), the second equality-only index type
(point lookups, advertises no range, declines range_scan, works through the
facade, facade surfaces the range error), `OrderedKey` total order (string,
mixed-numeric collapse, numeric range scan, round-trip), `IndexError` display,
and `IndexCapabilities` constructors.

`./format_code.sh` — green (exit 0): `cargo fmt --all`, `cargo clippy --workspace
--all-targets --all-features -D warnings`, latency-sim clippy, taplo all clean.

Coverage: `cargo llvm-cov` could not run in this worktree's shell
(`llvm-tools-preview` not installed here; `error: failed to find
llvm-tools-preview`). The CI coverage job (T-0005) computes the gate number. The
new module's 33 tests exercise every public method on both index implementations,
the full planner facade, every `Selectivity` branch, and the `OrderedKey` order —
no untested public path. The change is additive (new module + new ADR; lib.rs
gains one `pub mod` line), so existing coverage is not regressed.

## Review gate

- [ ] adversarial-reviewer sign-off (see docs/process/adversarial-review-loops.md)
- [ ] premortem-analyst sign-off (see docs/process/adversarial-review-loops.md)
- [x] `./format_code.sh` green
- [x] `cargo nextest run` green (or `cargo test` outside Nix shell)
- [x] coverage not regressed
- [x] board item updated to `in_review`

<!-- Reviewers: append your verdict block below this line per adversarial-review-loops.md -->
