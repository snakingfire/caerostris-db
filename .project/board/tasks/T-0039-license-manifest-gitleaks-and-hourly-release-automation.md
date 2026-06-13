---
id: T-0039
title: License manifest, gitleaks pre-commit, and hourly-release automation
type: task
status: blocked
priority: P2
assignee:
epic: EPIC-010
deps: []
rubric_refs: [12]
estimate: S
created: T0+0:20
updated: T0+1:05
---

## Context

Cat. 12 (process health) requires: gitleaks clean (pre-commit hook), every new
dependency recorded with its SPDX identifier + compatibility assessment in a
`docs/licenses/` manifest, and ≥1 hourly release cut per hour. This task wires that
hygiene automation. Independent of the engine — ready now. See `EPIC-010`,
`docs/process/open-source-guardrails.md`, `docs/process/release-hourlies.md`.

## Acceptance criteria
- [ ] gitleaks pre-commit hook configured and passing; a test/CI step confirms no secrets in commits.
- [ ] `docs/licenses/` manifest established: each dependency recorded with crate/package name, version, SPDX id, and a permissive-compatibility note; a check flags a new dep without a manifest entry.
- [ ] Hourly-release automation: a documented procedure or script that cuts a tagged release artifact at least once per hour during the run (per release-hourlies.md).
- [ ] A license-check step runs in CI (e.g. `cargo-deny` or equivalent, permissive-only allowlist).
- [ ] tests/checks added; coverage not regressed
- [ ] docs updated (licenses manifest + release procedure)
- [ ] `./format_code.sh` green

## Notes / log
Ready now: no engine dependency. Closes the Cat. 12 hygiene gaps (secrets, license
manifest, hourly releases) that the grader scores.

**T0+1:05 — BLOCKED by integrator (rebase conflict):**
Branch `worktree-wf_84c0f0c7-752-21` cannot be rebased onto `main` (currently at
`4a19941`). Conflict in `src/lib.rs`: both the branch (commit `69359c6`) and
`main` (commit `787d4f8 feat(query): add QueryStatistics side-effect surface for
TCK assertions`) added a `pub mod` declaration at the same location.

Branch delta: `pub mod licenses;`
Main delta:  `pub mod query;`

The fix is trivial (both lines belong in `src/lib.rs`; add both), but the protocol
requires the author to resolve, re-run `./format_code.sh` + tests, and re-request
review with the review-gate checkboxes reset to unchecked.

Steps for the author:
1. `cd .claude/worktrees/wf_84c0f0c7-752-21`
2. `git rebase main` — resolve conflict in `src/lib.rs` (keep both `pub mod licenses;` and `pub mod query;`)
3. `git add src/lib.rs && git rebase --continue`
4. `./format_code.sh` — must be green
5. `cargo nextest run` — must be green
6. Reset review-gate checkboxes in `PR.md` to unchecked
7. Re-request adversarial review + premortem
8. Set board item back to `in_review` once both reviewers re-approve
