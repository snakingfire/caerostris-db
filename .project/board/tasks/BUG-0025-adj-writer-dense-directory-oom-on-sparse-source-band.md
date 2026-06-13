---
id: BUG-0025
title: .adj writer allocates a dense offset directory over the full source-id span — panics/OOMs on a sparse band
type: bug
status: ready
priority: P1
assignee:
epic: EPIC-001
deps: []
rubric_refs: [2, 3]
estimate: M
created: T0+4:12
updated: T0+4:12
---

## Context

Found by the adversarial review of PR T-0008 (branch
`work/T-0008-implement-compressed-adjacency-list-edge-writ`).

`AdjShardWriter::finish` (`src/storage/adjacency.rs:331-362`) creates **one** shard
spanning `[min_src, max_src]` of whatever edges were added and emits a **dense** offset
directory — one 16-byte entry per id in `[lo, hi]`, *including every gap*
(`for offset in 0..band_width`). It does no banding/splitting; that was assumed to be a
caller's job (T-0009) but no such layer exists, and `finish` is the only entry point.

Two failure modes on realistic (sparse) node ids:
- `band_width = (src_band_hi - src_band_lo) + 1`; for a gap > `u32::MAX` (≈ 4.3e9 —
  trivially exceeded by ids in a 1B-node graph) `u32::try_from(band_width).expect(...)`
  **panics**.
- For a gap below `u32::MAX` but large (e.g. sources `0` and `10^9`), the writer attempts
  a ~16 GB directory. Proven during review: the test process was **killed (OOM/abort)
  with no output**.

ADR 0008 §3.1 (banded shards) and §6.4 (≤ 4 MiB shard-size tunable) are not realized; the
T-0008 AC "chunk granularity matches the spec's range-read access pattern" is not met for
any non-dense id distribution. Dense-band gaps also waste directory bytes the cost model
does not budget.

## Acceptance criteria
- [ ] The writer bands/splits edges by source-id span so a single `.adj` object's size is
      bounded (per ADR 0008 §6.4, a writer tunable, default ≤ 4 MiB) — never allocating
      proportional to the source-id *gap*.
- [ ] Sparse source-id distributions (e.g. sources `{0, 10^9, 2·10^9}`) serialise without
      panic or unbounded allocation; directory size is O(distinct sources or band width),
      not O(id gap).
- [ ] The `expect("band width fits u32 ...")` is replaced by a real error path (no panic on
      legal input).
- [ ] Round-trip + size tests for sparse bands; coverage not regressed; `./format_code.sh` green.

## Notes / log
Reported by adversarial-reviewer on the (stale duplicate) T-0008 PR
`work/T-0008-implement-compressed-adjacency-list-edge-writ`.

**Scope note (important):** the duplicate PR is being dropped (the canonical T-0008
already landed on `main` in `3c0bd9c` as `AdjacencyShardWriter`). The defect's *severity*
differs by implementation:
- **Dropped duplicate PR (`AdjShardWriter`):** severe — `finish()` *derives* the band from
  min/max of arbitrary added edges, so a sparse id set unavoidably forces a huge dense
  directory (proven: OOM-kill at a 10^9 gap; `u32::try_from(...).expect()` panic above
  `u32::MAX`).
- **Landed `main` (`AdjacencyShardWriter`):** mitigated but not eliminated — `new(...,
  src_band_lo, src_band_hi)` takes the band as a *caller* argument (caller owns the ≤ 4 MiB
  contract), but `finish()` still does `vec![(0,0,0); band_width]` (line ~345), so a caller
  that passes a wide band (or no banding layer yet enforcing ≤ 4 MiB) still blows up. There
  is no banding/splitter caller on `main` today.

So this BUG is: (a) a hard reason the duplicate PR cannot land, and (b) a real defensive gap
in the landed code — a writer should reject or bound a band whose dense directory would
exceed a sane cap rather than `vec!`-allocate proportional to the id span. Coordinate with
T-0009 (manifest partition map) on which layer owns banding. Rubric Cat. 2/3 (GATE).
