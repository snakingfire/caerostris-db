---
id: BUG-0028
title: Adjacency expand() errors (BadVarint) instead of early-aborting when the byte cap is below a block's leading degree varint
type: bug
status: in_progress
priority: P1
assignee: implementer-wf_3215ee4a-fcf-29
epic: EPIC-001
deps: []
rubric_refs: [2, 3]
estimate: S
created: T0+4:10
updated: T0+4:28
---

## Context

Found by adversarial review of T-0008 (`work/T-0008-adjacency-edge-writer-reader`,
landed on `main` via merge `3c0bd9c`). The CSR adjacency reader's
`AdjacencyShardReader::expand()` is supposed to honor a **hard per-GET byte cap**
that **truncates (early-aborts)** rather than fails — ADR 0008 §3.4, ratification
condition **C2**, and board AC #2 ("stop fetching once the LIMIT-driven frontier
is satisfied"). ADR 0008 §3.4 makes this load-bearing: "the executor tracks a
**running byte budget** ... caps the requested byte length at the **remaining
budget** ... and **truncates** ... once the LIMIT is satisfied or the budget is
exhausted."

The implementation honors the cap for the *neighbor entries*, but the
leading per-block `degree` varint is read **outside** the early-abort guard.

## The defect

In `src/storage/adjacency.rs`, `decode_block_prefix`:

```rust
let encoded_degree = cursor.read_varint()? as usize;   // <-- the `?` escapes
```

The per-entry decode loop already special-cases `BadVarint`/`Truncated` as a
clean early-abort ("ran off the capped buffer mid-entry — this is the early abort,
not corruption"). But the **leading degree varint** read above is not inside that
protection. When `expand` is called with `ExpandCap::bytes(n)` where `n` is
smaller than the block's leading degree varint (n = 0, or n = 1 for any block
whose degree >= 128), `want = block_len.min(n)` yields a 0- or 1-byte buffer, the
degree varint cannot be read, and `expand` returns
`Err(StorageFormatError::BadVarint)` instead of
`Ok(Expansion { neighbors: [], truncated: true, .. })`.

Reproduction (verified on the landed code with a scratch test):

| `ExpandCap::bytes(n)` | observed |
|---|---|
| n = 0 | `Err(BadVarint)`  (should be clean early-abort) |
| n = 1, degree >= 128 | `Err(BadVarint)` |
| n >= 2 | `Ok`, `truncated: true`, 0 neighbours (correct) |

## Why it matters

This is exactly the §3.4 / C2 budget-driven hard-cap path that the latency proof
leans on (F1/F2, decision 0015). When an executor has nearly exhausted its running
byte budget across a frontier, the **remaining budget** handed to the last
source(s) can legitimately be 0 or a few bytes — and the contract says that must
**early-abort**, not error. Today it surfaces as a `StorageFormatError` that the
executor cannot distinguish from real corruption ("fail-closed as if corrupt"),
turning a normal LIMIT/budget stop into a query failure. The in-envelope happy
path (per-hop share ≈ 5120 B) never hits it, which is why the existing 32 tests
miss it — there is **no test for `max_bytes` below a block's degree prefix**.

## Acceptance criteria

- [ ] `expand` with `ExpandCap::bytes(n)` for any `n` (including 0) returns
      `Ok(Expansion { truncated: true, .. })` with a (possibly empty) valid
      neighbour prefix — never `Err` — when the only reason the block can't be
      fully decoded is the byte cap.
- [ ] The leading `encoded_degree` varint read participates in the same
      early-abort handling as the entry loop (a `BadVarint`/`Truncated` while
      reading a *capped* buffer is truncation, not corruption). A genuinely
      corrupt full-buffer block must still fail closed (checksum already guards
      open()).
- [ ] Regression test: a parametric test over `max_bytes in {0, 1, 2, 3, ...}`
      asserts monotone, error-free early-abort; plus a degree >= 128 block at
      `max_bytes = 1`.
- [ ] `./format_code.sh` green; coverage not regressed.

## Notes / log
- T0+4:10 (adversarial-reviewer): filed from the T-0008 review. Verdict on that PR
  = changes_requested (review gate left unchecked). NB: T-0008 was *landed before*
  the review gate completed (merge 3c0bd9c with the adversarial/premortem boxes
  unchecked) — flagged separately as a process deviation; this BUG fixes the code
  defect regardless.
