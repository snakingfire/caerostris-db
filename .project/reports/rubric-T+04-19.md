# Rubric Report — T+04:19 (human-testing window)

Generated: 2026-06-13T22:42:50Z (T0 18:24Z). Graded vs committed main (fb54520, builds green). Grader cron `0c896bf3` (13th). Prev: T+03-58 (~36).

## Headline: overall ~41 (↑ from ~36). Storage layer materializing + cache landed; still climbing past feature-complete. ≥90 missed; Cat 4 the one big hole.

## Scoreboard (vs committed main)
| Cat | Name | W | Score | Evidence | Gate? |
|----:|------|--:|------:|----------|:----:|
| 1 | ACID & correctness | 14 | 28 | SPIKE-0002 + TLA+ ratified; **T-0009 manifest/stats/version-resolution LANDED** (commit substrate). T-0010 atomic manifest-swap still backlog. | ✓ |
| 2 | Storage format & commit | 12 | 48 | **T-0008 adjacency edge writer/reader + T-0009 manifest LANDED**; T-0007 node-property writer in_review; SPIKE-0003 format ADR 0008 ratified. Real on-object layout materializing; full graph roundtrip ~1 land away. | ✓ |
| 3 | Latency envelope + SLA | 14 | 78 | Envelope ADR + cost model + T-0014 sim (P99 889ms cache-off). | ✓ |
| 4 | openCypher (TCK %) | 12 | 0 | Harness wired + lexer/parser landed + a working minimal MATCH executor (demo), but TCK pass-rate still 0% (no full planner→executor over storage). The one big miss. | ✓ |
| 5 | Secondary indices | 7 | 55 | T-0022 trait+impl+extensibility + BUG-0019 (openCypher = semantics) + BUG-0020 (range selectivity gate). | |
| 6 | Fast aggregates | 5 | 2 | backlog (needs executor) | |
| 7 | Concurrency & attach modes | 8 | 8 | SPIKE-0005 + SPIKE-0009 ratified (design); no impl. | ✓ |
| 8 | Python bindings | 6 | 14 | T-0030 pyo3/maturin scaffold + CI; not yet open+query. | |
| 9 | Caching | 4 | 45 | **T-0033 optional cache wrapper LANDED** (resource-aware LRU around the object store). Cold-SLA-cache-off test to confirm for full credit. | |
| 10 | Tests/coverage/benches | 8 | 84 | 96.29% line coverage + integration on MinIO + 57 src / 15 test files. No headline-query criterion bench. | ✓ |
| 11 | Formal verification | 6 | 65 | TLA+ commit model + latency sim committed/ratified. | ✓ |
| 12 | Process health | 4 | 70 | 40 done; ADRs/TLA+/sim/coverage/demo/dashboard; gitleaks; MIT; board honest; main green throughout. | |
| | **OVERALL** | 100 | **~41** | Σ ≈ 4125/100 = 41.3 | |

## Expected vs actual
- Past the T+4:00 ≥90 target (MISSED). Actual ~41, still CLIMBING: 36 → **41** since feature-complete, driven by Cat 2 (25→48, storage layer), Cat 9 (0→45, cache), Cat 1 (24→28, manifest), Cat 5 (50→55).
- ~41 min to the hard deadline; lanes still landing.

## Gaps (named; tracked + building)
- **Cat 4 (TCK, 0) — the one big hole:** needs the full query engine (planner T-0018 → executor T-0019 over storage) wired to the harness. T-0019 still backlog (needs storage readers, now landing).
- **Cat 1 (ACID):** T-0010 atomic manifest-swap commit not started (manifest substrate now landed).
- **Cat 2:** T-0007 node writer in_review → lands the full graph roundtrip.
- Cat 6 aggregates, Cat 7 attach-mode impl, Cat 8 Python query path: unbuilt/scaffold.

## Notes
The product is real and demoable (insert→MATCH→return works; storage layer persisting to objects is landing). The hardest theory (latency theorem, TLA+ commit model) done; breadth — esp. a running TCK-passing engine — is the shortfall. main green; deps clean.
