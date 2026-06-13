# Pace & Deadline Ledger

> The wallclock contract. The **launch command stamps `T0`** (autonomous-run
> start) and fills the absolute times below. The `pace-marshal` and `rubric-grader`
> read this every cycle. Everyone is accountable to it.

## Clock (filled at launch)

- **T0 (autonomous run start):** `2026-06-13T18:24:00Z` (2026-06-13 11:24:00 PDT)
- **BUILD_HOURS:** 4   → **feature-complete / rubric-green target: T0+4:00 = `2026-06-13T22:24:00Z`** (15:24:00 PDT)
- **DEADLINE_HOURS:** 5 → **hard deadline (end of human testing window): T0+5:00 = `2026-06-13T23:24:00Z`** (16:24:00 PDT)
- Human testing window: **T0+4:00 → T0+5:00** (22:24:00Z → 23:24:00Z).

> The launch command (`/launch`) computes and writes the absolute timestamps. If
> you are reading placeholders, the run hasn't been launched yet.

## Checkpoint targets (overall rubric score expected by each marker)

These are *guides*, front-loaded toward de-risking the GATE categories. The grader
reports expected-vs-actual; the pace-marshal reacts.

| Marker | Wallclock | Expected overall | What should exist |
|-------:|-----------|-----------------:|-------------------|
| T0+0:20 | 18:44:00Z (11:44 PDT) | n/a (setup) | Steering ratified intent+rubric; board fully decomposed; `TASK-001` (latency envelope) in progress; crates skeleton building. |
| T0+0:40 | 19:04:00Z (12:04 PDT) | ~10 | Latency envelope **defined** + commit-protocol TLA+ **drafted** (Cat 3/11 design done); storage format spec drafted; TCK harness wired (Cat 4 starts counting). |
| T0+1:00 | 19:24:00Z (12:24 PDT) | ~20 | Storage writer/reader roundtrips a real graph on the mock; ACID commit happy-path; first hourly release. |
| T0+1:40 | 20:04:00Z (13:04 PDT) | ~35 | P1 openCypher reads passing TCK; B-tree index used by planner; snapshot reads. |
| T0+2:20 | 20:44:00Z (13:44 PDT) | ~50 | Latency cost-model+sim **proves** target on mock; aggregates; embedded read + writer modes. |
| T0+3:00 | 21:24:00Z (14:24 PDT) | ~65 | P2 writes+txns in TCK; all four attach modes; Python read bindings; coverage climbing. |
| T0+3:30 | 21:54:00Z (14:54 PDT) | ~78 | P3 breadth pushing TCK toward 100%; perf benches meet SLA; ≥90% coverage in sight. |
| T0+4:00 | 22:24:00Z (15:24 PDT) | **≥90, all GATEs ≥90, Cat4=100** | **Feature-complete. Ready for human testing.** |

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

- **First launch attempt** stamped T0 `2026-06-13T18:18:30Z` and armed crons
  `93bfbe05` / `04c26baa`, but that launch session ended (crons are session-only,
  so they died with it; `CronList`/`TaskList` confirmed nothing live). No work
  landed — board still at scaffold state, no reports/decisions.
- **Re-launched** `2026-06-13T18:24:00Z`: T0 re-stamped to now, markers refilled,
  hard deadline `2026-06-13T23:24:00Z`. Crons + mainspring re-armed below.
- **Crons armed (re-launch):** rubric-grader `0c896bf3` (`*/20 * * * *`);
  pace-marshal `47888961` (`*/10 * * * *`). Both session-only (live with this
  launch session — if it ends, re-run `/launch`).
- **Epoch 1 mainspring** kicked off at T0 by `/launch` (run id `wf_84c0f0c7-752`).
- **T+0:22 grade:** overall **6/100** (`.project/reports/rubric-T+00-22.md`) — ON TRACK
  / slightly ahead of the setup-phase expectation. Design+process leading; landed-artifact
  GATEs near floor (expected pre-code). AMBER watch: implementation not yet landing — hard
  re-check at T+0:40 (≥~10 expected; envelope spec / TLA+ draft / skeleton / TCK harness).
