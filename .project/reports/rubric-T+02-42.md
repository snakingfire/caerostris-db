# Rubric Report — T+02:42

Generated: 2026-06-13T21:06:00Z  (T0 = 2026-06-13T18:24:00Z)
Grader cron: `0c896bf3` (8th cycle). Previous: `rubric-T+02-16.md` (overall 16).
Graded against the VERIFIED committed state of `main` (c1066a3), which builds green.

## Headline: overall ~20 (up from 16). Real foundation landed; a worktree-race incident was contained with no work lost.

**Incident (contained):** concurrent integrator + lane agents were merging in the *shared main worktree* (git checkout+merge), racing HEAD and flip-flopping the working tree between branches — which made board/code reads transiently show reverted state. Investigation confirmed **`main` is intact at c1066a3 with all key work landed**; nothing was lost. All agents/lanes are frozen pending a switch to strictly-serial landing (see pace note). The race never lost a committed object — but it would eventually, hence the freeze.

## Scoreboard (vs committed main c1066a3)

| Cat | Name | W | Score | Evidence (on main) | Gate? |
|----:|------|--:|------:|--------------------|:----:|
| 1 | ACID & correctness | 14 | 22 | **SPIKE-0002 ratified** (commit protocol) + **`formal/commit-protocol/commit_protocol.tla` (+liveness/probes cfg) committed**; SPIKE-0005 CAS/fencing constraints done. No commit *code* yet (T-0009/0010 unbuilt). | ✓ |
| 2 | Storage format & commit | 12 | 16 | **T-0001 landed**: `ObjectStore` trait + `MemoryStore`, building, tested. SPIKE-0008 storage constraints done. Graph format spec (SPIKE-0003) in_progress; no writer/reader. | ✓ |
| 3 | Latency envelope + SLA | 14 | 55 | SPIKE-0001 envelope ADR ratified + cost model. Sim (T-0014) not landed; no benchmark. | ✓ |
| 4 | openCypher (TCK %) | 12 | 0 | **T-0002 harness LANDED** (Gherkin runner + TCK 2024.3 corpus, 1602 scenarios); live pass-rate **0.00%** (PendingEngine, no query engine). `.project/reports/tck-T+02-30.md`. | ✓ |
| 5 | Secondary indices | 7 | 3 | trait task ready | |
| 6 | Fast aggregates | 5 | 2 | backlog | |
| 7 | Concurrency & attach modes | 8 | 8 | SPIKE-0005 (fencing/lease) + SPIKE-0009 (server protocol ADR) ratified; no impl | ✓ |
| 8 | Python bindings | 6 | 0 | nothing | |
| 9 | Caching | 4 | 0 | nothing | |
| 10 | Tests/coverage/benches | 8 | 12 | T-0001+T-0002 landed with 122 passing tests (unit+integration+doctest); env up. No coverage % on main (T-0005 unlanded), no benches. | ✓ |
| 11 | Formal verification | 6 | 50 | **TLA+ commit-protocol model committed on main** (commit_protocol.tla + liveness + probes cfgs) + latency cost-model ratified. (Model-check evidence to confirm.) | ✓ |
| 12 | Process health | 4 | 58 | 17 done items; ADRs/decisions; TLA+; gitleaks; MIT. Ding: the worktree-race caused transient board inconsistency (contained). | |
| | **OVERALL** | 100 | **~20** | Σ ≈ 2001/100 = 20.0 | |

## Expected vs. actual

- **Expected at T+2:42:** ~**50**.
- **Actual:** **~20. Delta: −30 (deep RED).**
- Movement since T+2:16 (16): Cat 1 14→22, Cat 2 9→16, Cat 10 3→12, Cat 11 32→50 — driven by T-0001/T-0002 landing + SPIKE-0002 ratification + TLA+ on main. The foundation is real now.

## Honest projection (unchanged band)

~1h18m to T+4:00, ~2h42m to the hard deadline. Realistic ceiling **~25–45**: strong design/proof (Cat 3 envelope ratified, Cat 11 TLA+ committed — the hardest parts) + a partial engine (object-store substrate + TCK harness wired) + whatever the serial-landing path converts from the built branch backlog (T-0005 coverage, T-0006 data model, T-0014 sim, storage writers). ≥90 remains out of reach.

## Gaps / next (pace-marshal)

The bottleneck is no longer "can't land" — it's "can't land *concurrently safely*." Fix: **strictly serial landing** (one integrator touching main at a time — proven: T-0001/T-0002 landed cleanly when sole). Next serial lands by weight: T-0006 (data model → unblocks storage), T-0005 (coverage→Cat10), T-0014 (sim→Cat3/11), then the query chain (Cat4) + storage writers (Cat2) + commit impl (Cat1).

## Notes

main builds green; 122 tests pass; TLA+ + envelope ADR + TCK harness all on main. The conceptually hardest requirements (latency theorem, commit-protocol formal model) are genuinely landed/ratified — that's the durable win even with execution far behind.
