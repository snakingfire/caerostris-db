---
id: BUG-0020
title: Range selectivity on a non-empty equality-only index reports 0.0 (most selective), then probe errors
type: bug
status: in_review
priority: P2
assignee: implementer-wf_e9fceb87-27c-38
epic: EPIC-005
deps: []
rubric_refs: [5, 3]
estimate: S
created: T+3:40
updated: T+4:02
---

## Context

Found during adversarial review of T-0022 (already landed on `main`, commit
`ab5fc7a`).

In the blanket `PropertyIndex` impl (`src/index/mod.rs`), `selectivity` for an
`IndexQuery::Range` against an index that cannot range-scan does:
```
Err(_) => return Selectivity::from_fraction(0, total),
```
The code comment claims "the planner sees a 0-match (non-selective) estimate and
falls back to a scan." That reasoning is inverted: **lower selectivity = MORE
selective** (the type's own doc), so `from_fraction(0, total)` on a non-empty index
is `0.0` = the *most* selective value possible. A selectivity-driven planner using
the documented `is_at_least_as_selective_as(threshold)` test will therefore
**choose** this index for a range query — and then `probe` returns
`Err(IndexError::RangeUnsupported)`.

Reproduced against the landed code with a non-empty (100-entry) equality-only index:
```
range selectivity.fraction()          = 0
is_at_least_as_selective_as(0.5)       = true     <-- planner would pick it
probe(Range(..))                       = Err: "this index does not support ordered range scans"
```

The single regression-guard test (`planner_facade_surfaces_range_error_on_
equality_only_index`) uses an **empty** index, where `from_fraction(0,0)` returns
`1.0` (least selective) and masks the bug. The non-empty case is untested.

Impact: contradictory contract — "most selective" yet "unservable." Once T-0024 /
EPIC-002 select indices by this surface, a range predicate over a hash/full-text
index can be routed to an index that then errors (or, if the planner swallows the
error, silently returns no rows — a Cat. 4 wrong answer). Latent (planner is a
stub today), so P2; fix before T-0024.

## Acceptance criteria
- [ ] `selectivity` for a range query an index cannot serve does not report a
      misleadingly-selective value. Preferred: gate on `supports_range()` so a
      range query against an equality-only index is never deemed usable (report
      least-selective `1.0`, or make `selectivity` itself fallible / `Option`).
- [ ] Test with a **non-empty** equality-only index: a range query is not chosen by
      `is_at_least_as_selective_as`, and the contradiction (selectivity says "use it"
      while probe errors) cannot arise.
- [ ] Correct the inverted code comment in the blanket impl.
- [ ] `./format_code.sh` green; coverage not regressed.

## Notes / log
- T+3:40 filed by adversarial-reviewer during T-0022 re-review. Pairs with BUG-0019.
- T+3:58 implementer-wf_e9fceb87-27c-38: fixed TDD-first on `work/BUG-0020-range-selectivity-equality-only-index`
  (branch off latest main `feef7ea`). `selectivity` now gates the `Range` arm on
  `capabilities().supports_range` and reports least-selective `1.0` when a range
  query cannot be served; added `Selectivity::least_selective()`; corrected the
  inverted comment. RED→GREEN regression tests on a NON-EMPTY equality-only index.
  Full workspace suite 296/296 green; `./format_code.sh` green. PR.md filled;
  status -> in_review. Dispatching adversarial-reviewer + premortem-analyst.
- T+4:02 adversarial-reviewer: **APPROVE** (verdict in PR.md). Independently
  reproduced RED→GREEN (reverted prod arm → 2 regression tests fail "got 0" /
  "usable at threshold 0", over-correction guard stays green → restored). Verified
  `cargo build --lib`, `clippy --lib --all-features -D warnings`, `cargo test --lib`
  (202 pass) all clean. Attacks landed: none — symmetric Equals gap (no, lookup is
  mandatory), prefix-bypass (no, prefix=Range, gated), latency/ACID/security (not
  engaged / strictly safer). Reviewer checkbox ticked. Non-blocking notes: stale
  base (board-file deltas vs current main are NOT this branch's; land.sh rebases
  first — integrator confirm clean rebase); threshold-1.0 edge documented for the
  T-0024 planner author. Awaiting premortem-analyst sign-off before landing.
