---
id: BUG-0024
title: .adj writer overflows i64 on neighbour dst ids that straddle 2^63 (debug panic / silent release wrap)
type: bug
status: done
priority: P1
assignee: implementer-wf_e9fceb87
epic: EPIC-001
deps: []
rubric_refs: [2, 3]
estimate: S
created: T0+4:12
updated: T0+5:15
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

---
**T0+4:42 — claimed by implementer-wf_e9fceb87** (branch
`work/BUG-0024-adj-writer-i64-delta-overflow-on-high-node-ids`, based on latest `main`
`05463f1`). Verified the disposition note: the canonical `AdjacencyShardWriter` on `main`
encodes the delta with `wrapping_sub` (`src/storage/adjacency.rs:355`) and decodes with
`wrapping_add` (`:790`) — both purely in the `u64` domain, no `as i64` round-trip. **AC #1
is therefore already satisfied on `main`; a code fix would be a no-op.**

Rather than bare-`drop` and lose the value, I am delivering the genuinely-missing **AC #2**:
a round-trip regression test covering neighbour dst ids spanning the `2^63` boundary
(`{7, 2^63-1, 2^63, u64::MAX}` from one source). The existing `tests/adjacency_storage.rs`
suite never exercised dst ids above `~1e6`, so AC #3's "arbitrary directed typed edge sets"
claim was unproven for the full `u64` id space (Cat. 2 / Cat. 3). The test is the durable
form of the recorded lesson and converts BUG-0024 into a permanent regression guard. No
production code changes; test-only diff.

**T0+4:52 — in_review.** Branch `work/BUG-0024-adj-writer-i64-delta-overflow-on-high-node-ids`,
2 commits ahead of `main` (claim + test). PR.md filled. Test
`neighbor_dst_ids_spanning_2_63_boundary_round_trip` verified RED (panics
`attempt to subtract with overflow` at `adjacency.rs:356`) against a temporary signed-delta
variant and GREEN against the canonical `wrapping_sub` in both debug and release. Full suite
476/476 pass (was 475); `./format_code.sh` exit 0. Dispatching adversarial-reviewer +
premortem-analyst.

**T0+5:02 — premortem-analyst sign-off: approve.** Verified the diff is test-only (zero
production lines), so every P0 lens (corruption / SLA / split-brain / blast-radius / OSS
hygiene) is provably N/A. Confirmed landed code encodes the dst-id delta in the `u64`
domain (`adjacency.rs:355` `wrapping_sub`, `:787` `wrapping_add` — no `as i64` on the
neighbour path; buggy `AdjShardWriter` absent). Reproduced the guard's teeth: patched
production to the signed variant → test RED (`subtract with overflow` at `:355`), reverted
clean. Boundary test green in debug **and** release; full `adjacency_storage` suite 8/8;
`./format_code.sh` exit 0. Non-blocking: stale PR.md test-evidence prose; `drop`→`guard`
repurposing (a strict improvement, accepted). Pre-mortem box ticked in PR.md. Clear to land.

**T0+5:15 — Landed in commit 8717b31.** Both sign-offs verified (adversarial-reviewer approve T+4:58, premortem-analyst approve T+5:02). `./format_code.sh` green; adjacency_storage 8/8 pass (debug). Merged no-ff into main: `land: BUG-0024 adj writer i64-delta overflow on high node ids — boundary regression guard`. Status: done.
