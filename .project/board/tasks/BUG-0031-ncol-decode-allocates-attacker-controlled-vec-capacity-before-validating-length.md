---
id: BUG-0031
title: .ncol decode_value pre-allocates an attacker-controlled Vec/Map width — poisoned object triggers a memory-amplification DoS (not fail-closed on resource use)
type: bug
status: ready
priority: P2
assignee:
epic: EPIC-001
deps: [T-0007]
rubric_refs: [2, 1, 10]
estimate: S
created: T0+~4:40
updated: T0+~4:40
---

## Context

Found during the adversarial review of BUG-0026 (the recursion-*depth* fix on
`work/BUG-0026-ncol-encode-decode-of-nested-list-map-recurses-unb`). This is a
**different bug class** — allocation *width*, not recursion *depth* — and is
**pre-existing on `main`** (it predates BUG-0026; the BUG-0026 PR neither
introduces nor regresses it). It is filed separately so it is owned rather than
silently inherited, exactly as BUG-0030 split out the `.adj` depth sibling.

`src/storage/ncol.rs` `decode_value` reads a container element count `n` as a
`u64` straight from the (untrusted) object and immediately does
`Vec::with_capacity(n)` (LIST) / builds a `BTreeMap` over `0..n` (MAP) **before**
validating that the object actually contains `n` elements:

```rust
// src/storage/ncol.rs (main, ~line 383 / branch ~line 441)
tag::LIST => {
    // (BUG-0026 depth check here now)
    let n = get_u64(b, *at)? as usize;
    *at += 8;
    let mut items = Vec::with_capacity(n);   // <-- n is attacker-controlled
    for _ in 0..n { items.push(decode_value(b, at, depth + 1)?); }
    ...
}
```

A poisoned/corrupt `.ncol` column chunk declaring `n = u64::MAX` (or any large
value) makes the reader attempt a huge up-front allocation. The per-element loop
*would* eventually fail-closed with `Truncated`, but only **after** the
`with_capacity` allocation is attempted — so on a server serving reads this is a
remote, unauthenticated **memory-amplification DoS** (OOM / allocator abort) from
a tiny poisoned object; embedded, it can OOM the host process. ADR 0008 §8.2 /
the BUG-0014 "parse must not fail open" lesson require the reader to fail-closed
on a hostile object **without** trusting a length it has not validated.

Note BUG-0026's depth bound does **not** address this: the depth check guards
recursion only; a single top-level `LIST` with `n = u64::MAX` is depth 1 and
sails past the depth check straight into the over-allocation. The `.adj` codec
already partially mitigates the analogous case with `Vec::with_capacity(n.min(1024))`
(see BUG-0030 note) — the `.ncol` reader has no such cap.

## Acceptance criteria
- [ ] `decode_value` (and any sibling length-prefixed reads) must not allocate a
      collection sized by an unvalidated attacker-controlled length. Either cap
      the pre-allocation (e.g. `n.min(REASONABLE_CAP)` and grow on demand) or
      validate `n` against the remaining byte budget (each element needs ≥1 byte,
      so `n` cannot exceed the bytes left in the chunk) before reserving.
- [ ] A test feeds a poisoned chunk declaring a huge `n` with too few following
      bytes and asserts a typed fail-closed error (`Truncated`/`Malformed`) with
      no large allocation / OOM / abort.
- [ ] Apply the same fix to the LIST and MAP paths (and the MAP key-length read,
      which `get`s a slice of attacker length but is already bounds-checked).
- [ ] Document the "validate-length-before-allocate" rule in ADR 0008 §8.2 as a
      reader contract alongside the BUG-0026 depth bound.
- [ ] tests added; coverage not regressed; `./format_code.sh` green.

## Notes / log
Filed by adversarial-reviewer during the BUG-0026 review. Scope: `.ncol` reader
width pre-allocation. The analogous `.adj` width case is partially capped today
(`n.min(1024)`); evaluate folding both into one shared "self-describing value
codec" helper when BUG-0030 lands, to avoid a third copy of the same hardening.
