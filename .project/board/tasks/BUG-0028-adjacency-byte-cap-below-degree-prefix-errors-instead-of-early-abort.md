---
id: BUG-0028
title: Adjacency expand() errors (BadVarint) instead of early-aborting when the byte cap is below a block's leading degree varint
type: bug
status: done
priority: P1
assignee: implementer-wf_3215ee4a-fcf-29
epic: EPIC-001
deps: []
rubric_refs: [2, 3]
estimate: S
created: T0+4:10
updated: T0+4:54
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
- T0+4:30 (implementer-wf_3215ee4a-fcf-29): fixed on `work/BUG-0028-adjacency-byte-cap-below-degree-prefix-early-abort`
  (based on latest `main`). TDD: 3 RED byte-cap tests reproduced `Err(BadVarint)`
  on the landed code, then GREEN. `decode_block_prefix` now takes a `buffer_capped`
  flag; on a capped buffer a degree-varint that runs off the end returns
  `Ok((vec![], truncated))`; on a full buffer it still fails closed (checksum guards
  open()). Added `full_buffer_corrupt_degree_varint_fails_closed` (decoder-direct,
  pins the `!buffer_capped` Err arm) + integration test
  `exhausted_budget_on_last_source_early_aborts_not_errors`. Full suite 480/480
  green; `./format_code.sh` exit 0. PR.md filled; status → in_review; dispatching
  adversarial-reviewer + premortem-analyst.
- T0+4:36 (adversarial-reviewer): verdict **approve**. Re-ran in the worktree:
  `cargo nextest run storage::adjacency` 29/29, `--test adjacency_storage` 8/8,
  full suite 480/480, `./format_code.sh` exit 0 (worktree clean after). Hardest
  attack (corrupt overflowing varint slipping through the capped arm) does not
  land: such a byte pattern is unreachable on legitimately written data and
  corrupt blocks are gated by the open-time checksum; both `match` arms are pinned
  by `full_buffer_corrupt_degree_varint_fails_closed`. Reviewer box ticked in
  PR.md. Three non-blocking notes (open-vs-expand re-read window is pre-existing;
  BUG-0027 also edits adjacency.rs — land in sequence; add a MinIO n=0 case as
  follow-up). Still needs premortem-analyst sign-off before the integrator lands.
- T0+4:46 (premortem-analyst): verdict **approve** — premortem box ticked in PR.md.
  Read-path-only diff; worked all six lenses backwards. Every P0 (silent
  corruption, ACID/concurrency, latency-theorem, blast radius, OSS hygiene) is
  out-of-scope or mitigated by a verified guard: the open-time whole-object
  checksum (`open()` `get()`+`verify_checksum`) gates corruption before any
  `expand`; both stores enforce ranges strictly (no silent short-read can fake a
  cap); `buffer_capped = (want < block_len)` confines the new early-abort to the
  caller's deliberate sub-block fetch; the fail-closed arm is pinned by
  `full_buffer_corrupt_degree_varint_fails_closed`. The new `match` only catches
  decode errors (`BadVarint`/`Truncated`), never `StoreError`, so an S3 5xx still
  propagates. No new dependency / no format bump (`git diff --stat`: only
  adjacency.rs + adjacency_storage.rs + this item). Re-verified gates in the
  worktree: clippy `-D warnings` clean, full suite **480/480** (MinIO ran),
  `./format_code.sh` exit 0. Two non-blocking follow-ups recorded (pre-existing
  open-vs-expand re-read window; a MinIO n=0 integration case). No blocking
  findings — both gates now `approve`; ready for the integrator to land.
- T0+4:54 (integrator): Landed in commit 9a2df495 at T0+4:54. Both review gates verified approve in PR.md. ./format_code.sh exit 0. cargo nextest run 480/480 pass. Merged work/BUG-0028-adjacency-byte-cap-below-degree-prefix-early-abort into main with --no-ff; cargo build clean post-merge. Worktree removed. Branch deletion was denied by permission classifier (non-blocking; branch is fully merged).
