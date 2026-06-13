# Rubric Report — T+00:37

Generated: 2026-06-13T19:00:40Z  (T0 = 2026-06-13T18:24:00Z)
Grader cron: `0c896bf3` (2nd cycle). Previous report: `rubric-T+00-22.md` (overall 6).

## Scoreboard

| Cat | Name | Weight | Score | Evidence | Gate? |
|----:|------|-------:|------:|----------|:----:|
| 1 | ACID txns & correctness | 14 | 10 | **SPIKE-0005 complete** → `docs/specs/SPIKE-0005-commit-protocol-pre-ratification-constraints.md` (CAS primitive, fencing token, durability barrier); decision `0004`. SPIKE-0002 (protocol + TLA+) **claimed/in-progress** — no protocol spec finalized, no TLA+, no code. | ✓ |
| 2 | Storage format & S3 commit | 12 | 7 | SPIKE-0008 (storage falsification constraints) in-progress → `docs/specs/SPIKE-0008-storage-falsification-constraints.md`; decision `0001`. No format-layout ADR, no writer/reader. SPIKE-0003 still `backlog` (now **unblockable** — see Notes). | ✓ |
| 3 | Latency envelope + SLA | 14 | **48** | **SPIKE-0001 complete** → `docs/adr/0001-latency-selectivity-envelope.md` (749 lines: 5-param envelope `(s,F_tail,M_max,K,L_p99)`, B_max derivation, serial latency floor `K_min·L_p99`, max-of-M order statistic, out-of-envelope detection algo). SPIKE-0006 → `docs/specs/SPIKE-0006-…` pins L_p99=50ms, r=1. SPIKE-0007 → `docs/adr/0001-cold-start-benchmark-protocol.md`. **Status `proposed`** (steering sign-off pending, decision `0012`); no sim/benchmark yet. | ✓ |
| 4 | openCypher (TCK %) | 12 | **0** | TCK harness **not wired** (no `tests/tck/`) → floor 0. Definition pinned: decisions `0007`/`0008`. T-0002 READY, unclaimed. | ✓ |
| 5 | Secondary indices | 7 | 3 | T-0022 (trait) READY; no impl. | |
| 6 | Fast aggregates | 5 | 2 | T-0020 backlog; no impl. | |
| 7 | Concurrency & attach modes | 8 | 5 | Writer-fencing/lease constraints in `docs/specs/SPIKE-0005-…` (fencing token + durability barrier); T-0026/0027 backlog; no impl. | ✓ |
| 8 | Python bindings | 6 | 0 | No `python/`; T-0030/31/32 backlog. | |
| 9 | Caching | 4 | 0 | Nothing; T-0033/34 backlog. | |
| 10 | Tests/coverage/benches | 8 | 3 | CI configured (`.github/workflows/ci.yml`); integration env up (MinIO @ :9000). **No code → no tests, no coverage, no benches.** | ✓ |
| 11 | Formal verification | 6 | 20 | **Latency cost-model committed** (the analytical model in `docs/adr/0001-latency-selectivity-envelope.md` — a Cat-11 artifact); decisions `0005`/`0009`. **TLA+ commit/isolation model NOT drafted** (no `formal/` dir; SPIKE-0002 in-progress). Half the anchor-50 criteria met. | ✓ |
| 12 | Process health | 4 | 58 | 12 committed decisions/ADRs (`.project/decisions/`, `docs/adr/`); board honest; commits prefixed (`board:`/`pace:`/`report:`); gitleaks pre-commit; MIT, zero deps; CLAUDE.md current. **Docked: recurring ADR/decision ID collisions** (two `ADR-0001`, three `decision-0012`). No hourly release yet (first hour ends T+1:00 — not due). | |
| | **OVERALL** | **100** | **13** | Σ(score×weight)/100 = 1343/100 = 13.4 | |

## Gate status

| Gate | Score | T+0:40 leading indicator | Status |
|------|------:|--------------------------|--------|
| Cat 1 (ACID) | 10 | commit-protocol TLA+ drafted | AMBER — constraints done; TLA+/protocol spec pending (SPIKE-0002 in-prog) |
| Cat 2 (Storage) | 7 | storage format spec drafted | AMBER — SPIKE-0003 unblockable now (dep SPIKE-0001 done) |
| Cat 3 (Latency) | 48 | **envelope defined** | **GREEN — done & ahead** (envelope ADR committed) |
| Cat 4 (TCK) | 0 | TCK harness wired | **RED watch** — T-0002 ready, unclaimed; weight-12 GATE at floor |
| Cat 7 (Concurrency) | 5 | (design) | GREEN (design constraints landing) |
| Cat 10 (Tests) | 3 | crate skeleton building | AMBER — blocked behind T-0000 (env hardening, not done) |
| Cat 11 (Formal) | 20 | TLA+ drafted | AMBER — latency model done; TLA+ pending (SPIKE-0002) |

## Expected vs. actual

- **Expected overall at T+0:40 (19:04Z):** **~10**.
- **Actual overall at T+0:37:** **13**.
- **Delta:** **+3 (AHEAD).**
- **Pace status:** **ON TRACK / AHEAD of the T+0:40 checkpoint.** The lead is driven almost
  entirely by the **latency envelope ADR landing** (Cat 3: 8→48, weight 14 = +5.6 overall) —
  the project's signature GATE and highest-risk invariant is now *defined* on schedule. This
  directly discharges the pace-marshal's T+0:22 AMBER watch on the design side: **specs ARE
  landing.** Remaining risk is entirely on the **implementation** side (see below).

## Gaps to close (new board tasks filed this cycle)

**None filed — every gap is already tracked by an existing READY/in-progress task.** Filing
duplicates violates the no-overhead doctrine. The gaps and their existing owners:

- **Cat 4 (TCK, weight 12, score 0) — highest-leverage gap.** Tracked by **T-0002** (wire TCK
  Gherkin runner), READY P0, **design-independent, unclaimed**. *Flagged to pace-marshal:* the
  epoch has dispatched only researchers/steering so far; a **test-author/implementer must claim
  T-0002 this wave** — wiring it alone moves a weight-12 GATE off floor 0.
- **Cat 10 / crate skeleton.** T-0001 blocked behind **T-0000** (env hardening — genuinely
  incomplete: needs `tests/integration` harness + concurrency proof). T-0000 is READY,
  unclaimed; it is the foundational unblock for all code.
- **Cat 11 (TLA+).** Tracked by **SPIKE-0002**, in-progress (claimed). On track if it lands a
  `formal/*.tla` draft by ~T+0:50.
- **Cat 2 (storage spec).** **SPIKE-0003 is now unblockable** — its dep SPIKE-0001 completed
  this cycle. *Flagged to pace-marshal to flip `backlog`→`ready`.*

## Regressions vs. previous report (T+00:22, overall 6)

None. All categories held or rose. Movers: Cat 3 **8→48**, Cat 11 **5→20**, Cat 1 **5→10**,
Cat 7 **3→5**, Cat 2 **5→7**, Cat 12 **55→58**. No GATE regressed.

## Notes

- **The design wave is nearly complete; the implementation wave has not started.** At T+0:37
  there is still **zero compiled code** (src = scaffold `lib.rs`/`main.rs`; no `tests/`, no
  `formal/`, no `python/`; Cargo not yet a workspace). The score is ahead purely on design
  artifacts. The T+1:00 checkpoint (~20) requires *landed code* (storage writer/reader
  roundtrip, ACID happy-path, first hourly release) — that demands implementers, not just
  researchers. **This is the single most important thing for the next epoch to correct.**
- **Process defect (Cat 12): ID collisions.** Two files share `ADR-0001`
  (`…-latency-selectivity-envelope.md`, `…-cold-start-benchmark-protocol.md`); three share
  `decision-0012`. This recurs (cf. `BUG-0004` collision fix in commit `e33f35f`). *Flagged to
  docs-memory-curator:* renumber to unique IDs and add an allocation guard. Not filing a task
  to avoid creating yet another ID collision mid-epoch; the curator owns numbering integrity.
- No secrets; deps empty (license-clean by construction). gitleaks in pre-commit.
- Grader invoked on schedule (2nd `*/20` cycle). Cadence nominal.
