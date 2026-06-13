---
id: BUG-0030
title: .adj edge property value codec recurses unbounded — deep nesting aborts the process (SIGABRT), not fail-closed
type: bug
status: ready
priority: P1
assignee:
epic: EPIC-001
deps: [T-0008]
rubric_refs: [2, 1, 10]
estimate: S
created: T0+~4:35
updated: T0+~4:35
---

## Context

Found while fixing BUG-0026 (the identical defect in `src/storage/ncol.rs`).

`src/storage/adjacency.rs` carries its **own copy** of the self-describing
property-value codec for edge properties (the `.adj` CSR adjacency objects,
T-0008): `encode_value` (writer) and `decode_value` (reader) recurse once per
nesting level for `PropertyValue::List` / `PropertyValue::Map` with **no depth
bound** — exactly the BUG-0026 defect, in a different module and against the
`StorageFormatError` error type rather than `NcolError`.

```rust
// src/storage/adjacency.rs (current)
fn encode_value(out: &mut Vec<u8>, v: &PropertyValue) { … List/Map recurse … }
fn decode_value(cursor: &mut Cursor<'_>) -> Result<PropertyValue, StorageFormatError> {
    …
    value_codec::LIST => { for _ in 0..n { items.push(decode_value(cursor)?); } }
    value_codec::MAP  => { … let value = decode_value(cursor)?; … }
    …
}
```

Same two impacts as BUG-0026:

1. **Reader not fail-closed on untrusted bytes.** A crafted/corrupt `.adj`
   object whose edge-property column holds a deeply nested list/map makes the
   adjacency reader **abort the whole process** (stack overflow / SIGABRT)
   instead of returning a `StorageFormatError`. On a server serving reads this
   is a remote, unauthenticated DoS via a poisoned object; embedded, it kills
   the host process. ADR 0008 §8.2 / the BUG-0014 lesson require fail-closed.
2. **Writer aborts on legitimate ingest** of a moderately nested edge-property
   value, mid-transaction, with no recovery path.

Note `decode_value`'s `Vec::with_capacity(n.min(1024))` only caps the *width*
allocation, not the recursion *depth* — it does not address this bug.

## Acceptance criteria
- [ ] `decode_value` (adjacency) enforces an explicit maximum nesting depth and
      returns a typed `StorageFormatError` (fail-closed) when exceeded — never
      overflows the stack.
- [ ] `encode_value` (adjacency) enforces the same bound (or is made iterative);
      ingest of an over-deep edge-property value returns an error, never aborts.
- [ ] Reuse the same depth bound as the `.ncol` codec (`ncol::MAX_NESTING_DEPTH`
      = 64) so the two object formats share one contract; reference ADR 0008 §2.3.
- [ ] A test serialises/reads an over-deep value and asserts a typed error (no
      panic/abort); a test confirms a value at the bound round-trips.
- [ ] tests added; coverage not regressed; `./format_code.sh` green.

## Notes / log
Filed by implementer-wf_3215ee4a-fcf-27 while fixing BUG-0026. BUG-0026 scoped
its fix to `src/storage/ncol.rs` (its acceptance criteria name that file); this
sibling defect in the `.adj` codec is tracked separately to keep that PR small
and focused. The fix here should mirror the BUG-0026 approach (thread a `depth`
counter, reject before recursing).
