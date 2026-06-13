# Pace & Deadline Ledger

> The wallclock contract. The **launch command stamps `T0`** (autonomous-run
> start) and fills the absolute times below. The `pace-marshal` and `rubric-grader`
> read this every cycle. Everyone is accountable to it.

## Clock (filled at launch)

- **T0 (autonomous run start):** `2026-06-13T18:18:30Z` (2026-06-13 11:18:30 PDT)
- **BUILD_HOURS:** 4   → **feature-complete / rubric-green target: T0+4:00 = `2026-06-13T22:18:30Z`** (15:18:30 PDT)
- **DEADLINE_HOURS:** 5 → **hard deadline (end of human testing window): T0+5:00 = `2026-06-13T23:18:30Z`** (16:18:30 PDT)
- Human testing window: **T0+4:00 → T0+5:00** (22:18:30Z → 23:18:30Z).

> The launch command (`/launch`) computes and writes the absolute timestamps. If
> you are reading placeholders, the run hasn't been launched yet.

## Checkpoint targets (overall rubric score expected by each marker)

These are *guides*, front-loaded toward de-risking the GATE categories. The grader
reports expected-vs-actual; the pace-marshal reacts.

| Marker | Wallclock | Expected overall | What should exist |
|-------:|-----------|-----------------:|-------------------|
| T0+0:20 | 18:38:30Z (11:38 PDT) | n/a (setup) | Steering ratified intent+rubric; board fully decomposed; `TASK-001` (latency envelope) in progress; crates skeleton building. |
| T0+0:40 | 18:58:30Z (11:58 PDT) | ~10 | Latency envelope **defined** + commit-protocol TLA+ **drafted** (Cat 3/11 design done); storage format spec drafted; TCK harness wired (Cat 4 starts counting). |
| T0+1:00 | 19:18:30Z (12:18 PDT) | ~20 | Storage writer/reader roundtrips a real graph on the mock; ACID commit happy-path; first hourly release. |
| T0+1:40 | 19:58:30Z (12:58 PDT) | ~35 | P1 openCypher reads passing TCK; B-tree index used by planner; snapshot reads. |
| T0+2:20 | 20:38:30Z (13:38 PDT) | ~50 | Latency cost-model+sim **proves** target on mock; aggregates; embedded read + writer modes. |
| T0+3:00 | 21:18:30Z (14:18 PDT) | ~65 | P2 writes+txns in TCK; all four attach modes; Python read bindings; coverage climbing. |
| T0+3:30 | 21:48:30Z (14:48 PDT) | ~78 | P3 breadth pushing TCK toward 100%; perf benches meet SLA; ≥90% coverage in sight. |
| T0+4:00 | 22:18:30Z (15:18 PDT) | **≥90, all GATEs ≥90, Cat4=100** | **Feature-complete. Ready for human testing.** |

## Behind-pace doctrine

If actual < expected at a marker:

1. **Triage to gates.** Reallocate agents to the lowest GATE categories
   (1,2,3,4,7,10,11) first.
2. **Cut scope, not quality.** Drop/defer the lowest-weight non-gate work
   (Cat 9 caching, Cat 6 nice-to-haves) — record as `dropped` with reason.
3. **Parallelize the TCK tail** (Cat 4): throw more `implementer`+`test-author`
   agents at failing scenario buckets.
4. **Never** relax a GATE's acceptance criteria to make a number look good. Report
   the honest gap instead.

## Log
<Append pace observations here each marker; the grader/marshal also link reports.>

- **T0 stamped** `2026-06-13T18:18:30Z` by `/launch`. Markers filled; hard deadline
  `2026-06-13T23:18:30Z`.
- **Crons armed:** rubric-grader `93bfbe05` (`*/20 * * * *`); pace-marshal
  `04c26baa` (`*/10 * * * *`). Both session-only (live with the launch session).
- **Epoch 1 mainspring** kicked off at T0 by `/launch` (run id in launch hand-off).
