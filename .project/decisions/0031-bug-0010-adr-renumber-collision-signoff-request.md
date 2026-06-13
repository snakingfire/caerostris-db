# Decision 0031: BUG-0010 — ADR 0001 numbering collision resolved by renumber (steering sign-off request)

**Date:** 2026-06-13 (T0+~3:05)
**Agent:** docs-memory-curator
**Reversible:** yes (a renumber is a pure docs/`git mv` operation; reversible by another `git mv`)
**Status:** requesting steering sign-off — board item `BUG-0010` set to `in_review` for ratification next round.

## Decision

Renumber the cold-start benchmark-protocol ADR from `0001` to `0004` to resolve the
collision in which two ADRs both occupied `docs/adr/0001-*`:

- `docs/adr/0001-latency-selectivity-envelope.md` — the canonical, widely-referenced
  latency-envelope ADR (steering-ratified, decisions 0015 / 0017). **Stays at 0001.**
- `docs/adr/0001-cold-start-benchmark-protocol.md` (SPIKE-0007) → **renamed to
  `docs/adr/0004-cold-start-benchmark-protocol.md`.** `0004` is the next free ADR
  sequence number (0001/0002/0003 are taken). The ADR's decision content is unchanged.

## Rationale

- The ADR README mandates a **unique zero-padded sequence number per ADR**. Two ADRs at
  `0001` break the index, ambiguate cross-references, and risk the rubric-grader
  mis-attributing Cat. 3 / Cat. 10 evidence. This is the docs/board-hygiene defect tracked
  as BUG-0010 (Cat. 12), surfaced as finding **F3** of the SPIKE-0001 ratification
  (decision 0015) by `steering-formal-methods`.
- **Lower-churn fix.** The benchmark-protocol ADR had far fewer live inbound references than
  the envelope ADR (which is cited across decisions, reports, board items, and the
  `formal/latency-sim` crate). Renumbering the benchmark ADR touches the fewest living docs.
  This matches the explicit instruction in BUG-0010 ("Do NOT renumber
  `0001-latency-selectivity-envelope.md` — it is the canonical, widely-referenced artifact").
- **No design impact.** The renumber does not alter the ratified latency theorem, the
  envelope parameters, the commit protocol, or any other binding design. It is pure docs
  hygiene.

## What was changed

- `git mv docs/adr/0001-cold-start-benchmark-protocol.md docs/adr/0004-cold-start-benchmark-protocol.md`;
  updated its title (`ADR 0001` → `ADR 0004`) and added a renumber provenance note in its
  Status section.
- Live inbound references updated to `0004`:
  - `docs/process/testing-and-benchmarks.md` — 2 markdown links + 2 "ADR 0001" text mentions.
  - `formal/latency-sim/README.md` — 1 markdown link (the cold-start-benchmark cross-ref).
  - `formal/latency-sim/src/lib.rs` — 1 doc-comment ("ADR-0001 cold-start-benchmark Rule 4").
  - `.project/board/tasks/SPIKE-0007-*` — 2 live acceptance-criteria cross-references.
  - `docs/adr/0001-latency-selectivity-envelope.md` — finding F3 note marked **RESOLVED**.
- **Deliberately left unchanged** (correct as-is — they reference the *envelope* ADR, which
  stays `0001`): all other "ADR-0001" mentions in `formal/latency-sim` (Cargo.toml description,
  cost-model/§1.1/§3.1 references, tests), `docs/adr/0003-server-mode-network-protocol.md`,
  and envelope-cost-model references across board items and reports.
- **Deliberately preserved as historical record** (append-only logs accurate at time of
  writing): the `0001-cold-start-benchmark-protocol` mentions in `.project/decisions/0015`,
  `.project/decisions/0017`, `.project/reports/rubric-T+00-37.md`, and the SPIKE-0007
  Notes/log entry dated T0+~0:41. A dated note was appended to the SPIKE-0007 log recording
  the renumber.

## Verification

- `./format_code.sh` green in the isolated worktree (cargo fmt + clippy `-D warnings` on the
  workspace and on `formal/latency-sim`, taplo) — the lib.rs change is a doc-comment only.
- Post-change grep confirms no living doc references a `0001-cold-start-benchmark` path; the
  only remaining mentions are the intentionally-preserved append-only history entries.

## Alternatives rejected

- **Renumber the envelope ADR instead:** rejected — it would break ~6+ live references across
  decisions, reports, board items, and the latency-sim crate, for no benefit. Explicitly
  forbidden by BUG-0010.
- **Rewrite the append-only decision logs / reports** to the new number: rejected — those are
  append-only historical records; rewriting them would falsify the record of what was true at
  the time. The renumber note in ADR 0004 and this decision log document the mapping instead.

## Sign-off requested

Steering: please confirm the renumber is correct and complete (no design impact, lowest-churn
target, history preserved) so `BUG-0010` can move `in_review → done`. This is reversible docs
hygiene; no quorum-blocking design question is at stake.
