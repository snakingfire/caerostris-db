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
created: T0+~4:05
updated: T0+~4:05
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
Reported by adversarial-reviewer on T-0008 PR. Blocks T-0008 land (GATE Cat. 2/3).
Coordinate with T-0009 (manifest partition map) on which layer owns banding.
