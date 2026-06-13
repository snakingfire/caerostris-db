# Pace & Deadline Ledger

> The wallclock contract. The **launch command stamps `T0`** (autonomous-run
> start) and fills the absolute times below. The `pace-marshal` and `rubric-grader`
> read this every cycle. Everyone is accountable to it.

## Clock (filled at launch)

- **T0 (autonomous run start):** `<stamped at launch>`
- **BUILD_HOURS:** 4   → **feature-complete / rubric-green target: T0+4:00 = `<stamped>`**
- **DEADLINE_HOURS:** 5 → **hard deadline (end of human testing window): T0+5:00 = `<stamped>`**
- Human testing window: **T0+4:00 → T0+5:00**.

> The launch command (`/launch`) computes and writes the absolute timestamps. If
> you are reading placeholders, the run hasn't been launched yet.

## Checkpoint targets (overall rubric score expected by each marker)

These are *guides*, front-loaded toward de-risking the GATE categories. The grader
reports expected-vs-actual; the pace-marshal reacts.

| Marker | Wallclock | Expected overall | What should exist |
|-------:|-----------|-----------------:|-------------------|
| T0+0:20 | – | n/a (setup) | Steering ratified intent+rubric; board fully decomposed; `TASK-001` (latency envelope) in progress; crates skeleton building. |
| T0+0:40 | – | ~10 | Latency envelope **defined** + commit-protocol TLA+ **drafted** (Cat 3/11 design done); storage format spec drafted; TCK harness wired (Cat 4 starts counting). |
| T0+1:00 | – | ~20 | Storage writer/reader roundtrips a real graph on the mock; ACID commit happy-path; first hourly release. |
| T0+1:40 | – | ~35 | P1 openCypher reads passing TCK; B-tree index used by planner; snapshot reads. |
| T0+2:20 | – | ~50 | Latency cost-model+sim **proves** target on mock; aggregates; embedded read + writer modes. |
| T0+3:00 | – | ~65 | P2 writes+txns in TCK; all four attach modes; Python read bindings; coverage climbing. |
| T0+3:30 | – | ~78 | P3 breadth pushing TCK toward 100%; perf benches meet SLA; ≥90% coverage in sight. |
| T0+4:00 | – | **≥90, all GATEs ≥90, Cat4=100** | **Feature-complete. Ready for human testing.** |

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
