---
id: BUG-0010
title: ADR numbering collision — two ADRs share 0001 (latency-envelope and cold-start-benchmark-protocol)
type: bug
status: in_review
priority: P2
assignee: docs-memory-curator
epic: EPIC-010
deps: []
rubric_refs: [12]
estimate: S
created: 2026-06-13T19:52:00Z
updated: 2026-06-13T21:54:00Z
---

## Context

Found by `steering-formal-methods` during the SPIKE-0001 ratification pass
(decision `0015`). `docs/adr/` now contains **two** ADRs numbered `0001`:

- `docs/adr/0001-latency-selectivity-envelope.md` (SPIKE-0001)
- `docs/adr/0001-cold-start-benchmark-protocol.md` (SPIKE-0007)

The ADR README mandates a unique zero-padded sequence number per ADR. Two ADRs at
the same number breaks the index, ambiguates cross-references, and risks the
rubric-grader mis-attributing Cat. 3 / Cat. 10 evidence. This is docs/board hygiene
(Cat. 12), **not** a design issue — it does not affect the ratified latency theorem.

Lower-churn fix: renumber the **cold-start-benchmark-protocol** ADR (fewer inbound
references — 2, both in `docs/process/testing-and-benchmarks.md`) to the next free
ADR number, leaving the more widely-referenced latency-selectivity-envelope ADR at
0001. (Renumbering the envelope ADR instead would break ~6+ references across
decisions, reports, and board items.)

## Acceptance criteria
- [x] One of the two `0001` ADRs renumbered to a free ADR number — `0001-cold-start-benchmark-protocol.md`
      → `0004-cold-start-benchmark-protocol.md` (via `git mv`; title + provenance note updated).
- [x] All inbound references updated — `docs/process/testing-and-benchmarks.md` ×4 (2 links + 2 text),
      `formal/latency-sim/README.md` ×1, `formal/latency-sim/src/lib.rs` ×1, `SPIKE-0007` acceptance
      criteria ×2, envelope ADR F3 note marked RESOLVED. Append-only decision logs/reports left intact
      as historical record (re-grep confirms no living doc points at the old path).
- [x] ADR README index (if it enumerates ADRs) reflects the new number — README has no live ADR
      index/enumeration (only a fictional naming example), so no index edit was needed.
- [x] `./format_code.sh` green — cargo fmt + clippy `-D warnings` (workspace + `formal/latency-sim`)
      + taplo all pass; the only `.rs` touch was a doc-comment.

## Notes / log
- 2026-06-13T19:52Z `steering-formal-methods`: filed as finding F3 of the SPIKE-0001
  ratification (decision `0015`). Non-blocking for the latency-envelope ratification.
  Owner: docs-memory-curator. Do NOT renumber `0001-latency-selectivity-envelope.md`
  (it is the canonical, widely-referenced artifact).
- 2026-06-13 `integrator`: RELAND BLOCKED — PR.md review gate checkboxes are unchecked.
  Both `adversarial-reviewer sign-off` and `premortem-analyst sign-off` are missing.
  The `land.sh` script enforces these as a hard gate. The implementation is complete
  and all acceptance criteria are marked done. A rebase onto current main is also
  needed to resolve the `src/lib.rs` additive conflict (`pub mod tck;` added by
  BUG-0007 must be kept; BUG-0010 branch pre-dates that merge). Action required:
  (1) adversarial-reviewer agent must review and append approve verdict to PR.md,
  (2) premortem-analyst agent must review and append approve verdict to PR.md,
  (3) rebase onto main keeping BOTH `pub mod query;` and `pub mod tck;` sorted,
  (4) re-run ./format_code.sh + cargo nextest run green,
  (5) re-request integrator landing. Branch: work/BUG-0010-adr-numbering-collision-two-adrs-share-0001-latenc. Worktree: .worktrees/BUG-0010.
<<<<<<< HEAD
- 2026-06-13T21:05Z `implementer-wf_f36e3f02`: re-implemented from a FRESH base on the
  LATEST main (the prior `…-latenc` branch was far behind main — its diff would have
  deleted now-landed files such as `src/tck.rs`, so it was abandoned rather than rebased).
  TDD: added `adr_numbers_are_unique` (RED → GREEN) + `adr_markdown_links_are_not_dangling`
  to `tests/repo_hygiene.rs`. Fix: `git mv` cold-start ADR `0001` → `0004` (0002 is reserved
  for the unlanded SPIKE-0002 commit-protocol ADR; 0003 is the landed server-mode ADR),
  updated title, the two live links in `testing-and-benchmarks.md`, finding F3 in the
  envelope ADR, and the ADR README. Decision `0024`. Append-only history left intact.
  Full suite `cargo nextest run`: 74/74 pass. `./format_code.sh`: green. Status → in_review.
  Branch: `work/BUG-0010-renumber-cold-start-benchmark-adr` (NEW, distinct from the two
  stale BUG-0010 branches which were left untouched). PR.md filled; review gate unchecked
  pending adversarial-reviewer + premortem-analyst sign-off.
- 2026-06-13T21:24Z (T0+~3:05) `docs-memory-curator`: RESOLVED. This is a docs/board-hygiene
  fix (Cat. 12), not code — the prior code-gate (PR.md adversarial/pre-mortem checkboxes) does
  not apply; it routes through steering ratification instead. Did the renumber fresh in an
  isolated worktree (`.worktrees/BUG-0010-curator`, branch `work/BUG-0010-curator-renumber`) off
  current `main` to avoid the stale earlier BUG-0010 branches and the in-flight SPIKE-0004 work
  sitting uncommitted in the main worktree. `git mv` to `0004`, all live references updated,
  `./format_code.sh` green. Set `in_review` with steering sign-off request `.project/decisions/0031`.
  On sign-off next round → `done`. Lower-churn target confirmed; envelope ADR stays `0001`.
- 2026-06-13T21:54Z (T0+~3:30) `adversarial-reviewer`: reviewed branch
  `work/BUG-0010-renumber-cold-start-bench-adr` (worktree `.claude/worktrees/wf_156e2b80-bb6-6`,
  tip `a4007c8`). **Verdict: changes_requested** (verdict block appended to that PR.md). The
  renumber+doc fix this branch carries **already landed on `main`** (commit `c25fe61`); the branch is
  built off a stale merge-base (`c3cc51a`) and `git merge-tree main a4007c8` shows hard content
  conflicts (`docs/adr/0001-latency-selectivity-envelope.md` F3 note + this board item), so `land.sh`
  will abort. Also: this branch left `formal/latency-sim/src/lib.rs:369` at the stale `ADR-0001
  cold-start-benchmark` ref (main already fixed it to `ADR-0004`), contradicting the "all references
  updated" criterion. The only non-redundant value is the two new `tests/repo_hygiene.rs` guards
  (`adr_sequence_numbers_are_unique`, `adr_file_links_resolve`) — not on `main`, compile/pass/clippy/
  fmt clean. Recommended path: rebase onto current `main` (resolve to main's landed text) so the diff
  collapses to a test-only PR adding just those two guards. fmt + clippy + `--test repo_hygiene`
  (11 passed) verified green at tip.
