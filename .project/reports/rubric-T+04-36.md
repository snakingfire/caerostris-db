# Rubric Report — T+04:36  (closing grade; ~23m to hard deadline 23:24Z)

Generated: 2026-06-13T23:00:44Z (T0 18:24Z). Graded vs committed main (603c429, builds green). Grader cron `0c896bf3` (14th). Prev: T+04-19 (~41).

## Headline: overall ~43. Full storage layer + planner landed; ≥90 MISSED. Cat 4 (TCK=0, no executor) is the dominant miss. A real, demoable, formally-grounded DB.

## Scoreboard (vs committed main)
| Cat | Name | W | Score | Evidence | Gate? |
|----:|------|--:|------:|----------|:----:|
| 1 | ACID & correctness | 14 | 30 | SPIKE-0002 + TLA+ ratified; **T-0009 manifest/version-resolution landed** (commit substrate). T-0010 atomic manifest-swap commit: ready, not landed. | ✓ |
| 2 | Storage format & commit | 12 | 62 | **FULL storage layer landed**: T-0001 ObjectStore, T-0006 model, T-0007 node writer/reader, T-0008 edge writer/reader, T-0009 manifest, SPIKE-0003 format ADR 0008. **Graph roundtrip to real S3 objects proven** (demo-minio: persist 13 objects → read back). Atomic manifest-swap (T-0010) pending. | ✓ |
| 3 | Latency envelope + SLA | 14 | 78 | Envelope ADR ratified + analytical cost model + T-0014 discrete-event sim (P99 889ms in-envelope, cache OFF). No real-engine benchmark (no executor). | ✓ |
| 4 | openCypher (TCK %) | 12 | 0 | Harness wired (1602 scenarios) + lexer (T-0017) + planner (T-0018) landed, BUT executor (T-0019) backlog → **TCK pass-rate 0.00%**. THE dominant miss. (A minimal MATCH executor works in the demo, just not TCK-wired.) | ✓ |
| 5 | Secondary indices | 7 | 55 | T-0022 pluggable trait + impl + extensibility + openCypher = semantics. | |
| 6 | Fast aggregates | 5 | 2 | backlog (needs executor). | |
| 7 | Concurrency & attach modes | 8 | 8 | SPIKE-0005/0009 ratified (design); no attach-mode impl (T-0027 backlog). | ✓ |
| 8 | Python bindings | 6 | 14 | T-0030 pyo3/maturin scaffold + CI; open+query (T-0031) backlog. | |
| 9 | Caching | 4 | 45 | T-0033 optional resource-aware cache wrapper landed. | |
| 10 | Tests/coverage/benches | 8 | 84 | 96.29% line coverage (cargo-llvm-cov) + MinIO integration suite + 61 src files. No headline-query criterion bench (no engine). | ✓ |
| 11 | Formal verification | 6 | 65 | TLA+ commit-protocol model + latency sim committed/ratified. | ✓ |
| 12 | Process health | 4 | 70 | 43 done; ADRs/TLA+/sim/coverage/demos/dashboard; gitleaks; MIT; board honest; main green throughout. | |
| | **OVERALL** | 100 | **~43** | Σ ≈ 4321/100 = 43.2 | |

## Expected vs actual (final)
- Target T+4:00: ≥90, all GATEs ≥90, Cat4=100. **Actual ~43. MISSED.**
- Trajectory after the recovery: 20→25→34→36→41→**43**. Steady climb once race-free landing was in place, but the lost first ~70 min (orchestration defects) capped total breadth.

## What was delivered (honest wins)
- **A working, recordable, object-storage-native demo** — `./scripts/demo-minio.sh`: empty bucket → insert a 6-node/7-edge graph → **13 real S3 objects in MinIO** → read back → 4 openCypher MATCH queries (incl. multi-property filter, one-hop traversal, WHERE). Verified end-to-end.
- **The two hardest, highest-risk requirements SOLVED:** latency selectivity-envelope theorem proven in simulation (cold P99 889ms, cache OFF) + S3 commit protocol formally modeled in TLA+ (ratified, 7 falsification attacks survived).
- **Complete storage layer** persisting graph data to object storage; real Cypher lexer+parser+planner; pluggable index trait; optional cache; **96.29% test coverage** on a green workspace.

## Honest gaps (named; not filing dead tasks at T-23m)
- **Cat 4 (TCK=0):** executor (T-0019) never landed → no TCK pass-rate. Biggest weight lost.
- **Cat 1:** atomic manifest-swap commit (T-0010) not landed (substrate is).
- **Cat 7 attach modes, Cat 6 aggregates, Cat 8 Python query:** unbuilt/scaffold.

## Retrospective (for the record)
The autonomous swarm lost its first ~70 minutes to a chain of orchestration defects (epoch land-barrier, broken multi-lane claim coordination, a `name=`-cache serving a stale orchestrator, and a shared-worktree merge race). Once landing was made race-free (merge-while-on-main + atomic claims + focused integrators), throughput became reliable and the score climbed steadily — but too late to reach feature-complete. The conceptual core is sound and demoable; breadth (a TCK-passing engine, all attach modes, Python) is the shortfall, stated openly on the board.
