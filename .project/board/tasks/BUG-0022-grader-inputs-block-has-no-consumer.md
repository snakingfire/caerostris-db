---
id: BUG-0022
title: GRADER_INPUTS CI-log block has no automated consumer; wire it or write a coverage-*.md evidence file
type: bug
status: blocked
priority: P2
assignee:
epic: EPIC-009
deps: []
rubric_refs: [10, 12]
estimate: S
created: T+3:30
updated: T+4:51
---

## Context

Filed by `premortem-analyst` during the post-land pre-mortem of **T-0005**
(`cargo-llvm-cov coverage + GRADER_INPUTS`, landed `0dd2f2d`). Not a defect
introduced by the diff — it is faithful to the T-0005 acceptance criteria — but a
latent operational gap that should be tracked rather than left silent.

T-0005's CI `coverage` job emits a structured `GRADER_INPUTS:` block
(`coverage_pct` / `test_pass` / `tck_pass_rate`) to the CI log and step summary.
The board item and `docs/process/ci-grader-inputs.md` assert "the rubric-grader's
evidence-scraper parses this block from CI logs". The **shipped** `rubric-grader`
agent (`.claude/agents/rubric-grader.md`) does not scrape CI logs — it reads
evidence from *files*: `.project/reports/coverage-*.md` for Cat. 10 and
`.project/reports/tck-latest.json` for Cat. 4 (rubric-grader.md:49). So the
`GRADER_INPUTS` block currently has **no automated consumer**.

Impact is bounded: the genuinely load-bearing path — the canonical
`tck-latest.json` the grader actually reads — *is* correctly wired and regenerated
by CI, so Cat. 4 evidence flows. The gap is that the coverage number emitted to
the CI log is not turned into the `.project/reports/coverage-*.md` file the grader
reads for Cat. 10; that file is presently produced by the grader cron itself, not
by this CI job. No corruption, ACID, latency, or security impact — metrics-wiring
only. Reference: PR pre-mortem in `.claude/worktrees/wf_156e2b80-bb6-3/PR.md`
([INTERFACE] / [OPERATIONAL] notes) and adversarial-reviewer [INTERFACE] note.

## Acceptance criteria
- [ ] Either (a) the `rubric-grader` parses the `GRADER_INPUTS:` block from the
      archived CI log/step-summary, **or** (b) the `coverage` CI job writes a
      committed/archived `.project/reports/coverage-<marker>.md` in the exact shape
      the grader reads for Cat. 10 — pick one and make the producer/consumer agree.
- [ ] While here, two metrics-accuracy refinements from the T-0005 pre-mortem:
      (i) the test-tally step uses `cargo test --workspace` so multi-member
      workspaces are not under-counted; (ii) `grader-inputs.sh` normalises a
      non-numeric coverage (e.g. jq `null`) to `0` so the emitted `coverage_pct`
      is never the literal `null` (the gate is already fail-safe; this is display).
- [ ] tests added/updated; coverage not regressed.
- [ ] docs (`docs/process/ci-grader-inputs.md`) reconciled so the stated consumer
      matches reality.
- [ ] `./format_code.sh` green.

## Notes / log
- **T+3:30 (premortem-analyst):** filed during T-0005 post-land pre-mortem.
  T-0005 itself approved (no P0 blocker); this is the non-blocking follow-up. The
  coverage *gate* and the TCK file path are correct and fail-safe; only the log
  block's consumer wiring and two cosmetic metrics refinements remain.
- **T+4:10 (integrator):** BLOCKED — reland of `work/BUG-0022-coverage-report-consumer`
  attempted but halted. The adversarial reviewer filed **changes_requested** (T+4:07,
  branch worktree `.claude/worktrees/wf_156e2b80-bb6-47`). Core finding: the
  `coverage-ci-*.md` evidence file is gitignored+artifact-only; the `rubric-grader`
  reads only the committed working tree; no artifact-download/commit-back mechanism
  exists — GRADER_INPUTS numbers still reach no automated consumer. AC (a)
  "make the producer/consumer agree" is unmet. `./format_code.sh` green; 368 tests
  pass; conflict is additive-only — but the review gate is the blocker.
  Author must address the adversarial finding, commit a fix in the worktree, and
  re-request both adversarial-reviewer and premortem-analyst sign-off (checkboxes
  reset to unchecked). Branch preserved in `.claude/worktrees/wf_156e2b80-bb6-47`.
- **T+4:51 (adversarial-reviewer):** **APPROVE** for branch
  `work/BUG-0022-committed-coverage-consumer` (tip `a730cdb`). The prior T+4:07
  changes_requested finding is fully resolved: instead of a gitignored/artifact-only
  file, `scripts/release-hourly.sh` step 7b now calls a new
  `scripts/release-coverage-report.sh` which renders the file via
  `grader-inputs.sh --write-report` AND **git-commits** it to
  `.project/reports/coverage-hourly-<N>.md` — the exact committed-working-tree glob
  the shipped `rubric-grader` reads for Cat. 10 (`rubric-grader.md:38`). Verified the
  end-to-end wire in a throwaway git repo: file lands git-tracked, matches the
  `coverage-*.md` glob, embeds the verbatim GRADER_INPUTS block + a human table, and
  leaves a clean tree. AC (i) `cargo test --workspace` tally confirmed to sum across
  all workspace members; AC (ii) null/empty/garbage coverage normalises to `0` (never
  literal `null`), valid decimals pass through. Bonus correctness fix: CI step now
  `set -o pipefail` so the coverage gate's exit propagates through `| tee` (it was
  previously masked — the gate was effectively a no-op in CI). Checks green:
  `./format_code.sh` exit 0 (fmt+clippy -D warnings+taplo clean); 541/541 workspace
  tests pass (1 benign leaky); 30+16 shell assertions + 5 Rust integration tests pass.
  Coverage not regressed (additions are shell + `#[test]` only). Attacks attempted —
  see below; none landed as blockers. Two non-blocking observations filed (release-time
  cargo duplication; marker path-traversal robustness in the general-purpose helper,
  unreachable via the only caller). Reviewer-gate box should be ticked in the
  regenerated PR.md (the original worktree `wf_156e2b80-bb6-76` was pruned).
  Premortem-analyst sign-off still required before the integrator lands.
