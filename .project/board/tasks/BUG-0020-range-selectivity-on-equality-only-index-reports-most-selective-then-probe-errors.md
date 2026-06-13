---
id: BUG-0020
title: Range selectivity on a non-empty equality-only index reports 0.0 (most selective), then probe errors
type: bug
status: ready
priority: P2
assignee:
epic: EPIC-005
deps: []
rubric_refs: [5, 3]
estimate: S
created: T+3:40
updated: T+4:04
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
- T+4:04 premortem-analyst: APPROVE on PR `work/BUG-0020-range-selectivity-on-a-non-empty-equality-only-ind`
  (worktree `wf_156e2b80-bb6-46/.worktrees/BUG-0020`). Verified locally: build +
  clippy `-D warnings` + fmt clean, 202/202 lib tests (37/37 `index::`). Pure
  in-memory selectivity-estimator fix; no S3/commit/lease/GC surface, no dep change.
  Premortem box ticked in PR.md. NON-BLOCKING integrator notes: (1) sibling BUG-0019
  (ready) edits the same `selectivity`/`probe` match blocks and still retains the
  buggy `from_fraction(0,total)` range arm — second-to-land must keep BOTH guard arms
  and re-run the other's regression tests post-rebase; (2) a DUPLICATE BUG-0020 PR
  exists (`work/BUG-0020-range-selectivity-equality-only-index`, `wf_e9fceb87-27c-38`,
  both gates already ticked) — land one, `dropped` the loser, do not double-close.
