---
id: BUG-0024
title: .adj writer overflows i64 on neighbour dst ids that straddle 2^63 (debug panic / silent release wrap)
type: bug
status: ready
priority: P1
assignee:
epic: EPIC-001
deps: []
rubric_refs: [2, 3]
estimate: S
created: T0+4:12
updated: T0+4:12
---

## Context

Found by the adversarial review of PR T-0008 (branch
`work/T-0008-implement-compressed-adjacency-list-edge-writ`).

`AdjShardWriter::encode_block` (`src/storage/adjacency.rs:412-414`) encodes the CSR
neighbour list as ascending zig-zag delta-varint dst ids, computing
`delta = (n.dst.get() as i64) - prev` in `i64`. `NodeId` is `pub u64`
(`src/model/node.rs:17`) and is engine-assigned; nothing constrains ids to `< 2^63`.

When two neighbour dst ids straddle the `u64`→`i64` sign boundary — e.g. `2^63 - 1`
(`= i64::MAX`) followed by `2^63` (`= i64::MIN`), which sort **adjacent** in `u64`
space — the subtraction `i64::MIN - i64::MAX` overflows.

## Reproduction (proven during review)

A writer with edges `5 -> (2^63 - 1)` and `5 -> 2^63`:
- **Debug/test build:** panics — `attempt to subtract with overflow` at
  `src/storage/adjacency.rs:413`.
- **Release build:** wraps silently; the round-trip happens to survive only because
  `decode_block` mirrors the same wrapping arithmetic — undocumented and unasserted.

The T-0008 round-trip property test never reaches this id range
(`tests/adjacency_storage.rs:130` caps `dst = g.below(10_000)`), so AC #3's "arbitrary
directed typed edge sets" claim is unproven for the full `u64` id space.

## Acceptance criteria
- [ ] dst-id deltas are encoded in the unsigned `u64` domain (e.g. `dst.wrapping_sub(prev)`
      on the ascending list, since the list is sorted so the delta is always ≥ 0 in u64),
      with no `as i64` round-trip — no overflow possible for any `u64` pair.
- [ ] A round-trip test covers neighbour dst ids spanning the `2^63` boundary
      (e.g. `{7, 2^63 - 1, 2^63, u64::MAX}` from one source) and passes in both debug and release.
- [ ] Coverage not regressed; `./format_code.sh` green.

## Notes / log
Reported by adversarial-reviewer on the **stale duplicate** T-0008 PR
`work/T-0008-implement-compressed-adjacency-list-edge-writ` (`AdjShardWriter`).

**Scope: PR-specific.** The canonical T-0008 that landed on `main` (`3c0bd9c`,
`AdjacencyShardWriter`) already encodes the delta with `wrapping_sub` in the `u64` domain
(`src/storage/adjacency.rs:355` on `main`) and is **not** affected by this overflow. This
bug is therefore a defect of the dropped duplicate only. Recommended disposition: **drop**
alongside the duplicate PR (see PR.md "OVERRIDING PROCESS FINDING"). Kept on the board so
the lesson — never round-trip `u64` node ids through `i64` for delta encoding — is recorded;
if the duplicate is closed without merge, mark this `dropped`.
