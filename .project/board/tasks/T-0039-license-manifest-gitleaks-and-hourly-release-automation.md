---
id: T-0039
title: License manifest, gitleaks pre-commit, and hourly-release automation
type: task
status: done
priority: P2
assignee: integrator
epic: EPIC-010
deps: []
rubric_refs: [12]
estimate: S
created: T0+0:20
updated: T0+2:15
---

## Context

Cat. 12 (process health) requires: gitleaks clean (pre-commit hook), every new
dependency recorded with its SPDX identifier + compatibility assessment in a
`docs/licenses/` manifest, and ≥1 hourly release cut per hour. This task wires that
hygiene automation. Independent of the engine — ready now. See `EPIC-010`,
`docs/process/open-source-guardrails.md`, `docs/process/release-hourlies.md`.

## Acceptance criteria
- [x] gitleaks pre-commit hook configured and passing; a test/CI step confirms no secrets in commits.
- [x] `docs/licenses/` manifest established: each dependency recorded with crate/package name, version, SPDX id, and a permissive-compatibility note; a check flags a new dep without a manifest entry.
- [x] Hourly-release automation: a documented procedure or script that cuts a tagged release artifact at least once per hour during the run (per release-hourlies.md).
- [x] A license-check step runs in CI (e.g. `cargo-deny` or equivalent, permissive-only allowlist).
- [x] tests/checks added; coverage not regressed
- [x] docs updated (licenses manifest + release procedure)
- [x] `./format_code.sh` green

## Notes / log
Ready now: no engine dependency. Closes the Cat. 12 hygiene gaps (secrets, license
manifest, hourly releases) that the grader scores.

- **T+0:40 (implementer-wf_84c0f0c7-752-21):** claimed; built TDD-first in worktree
  branch `worktree-wf_84c0f0c7-752-21`. Landed:
  - `src/licenses.rs` + `tests/license_manifest.rs`: self-contained license-manifest
    check (parses `Cargo.lock` vs `docs/licenses/manifest.toml`, permissive-only SPDX
    allow-list). Flags any dep missing from / non-permissive in the manifest. Runs under
    `cargo test`.
  - `docs/licenses/manifest.toml` + `docs/licenses/README.md`: the ledger (empty — zero
    third-party deps today) + the two-layer hygiene doc.
  - `.gitleaks.toml` (extends default ruleset), wired in CI `secret-scan` job + existing
    pre-commit hook. `gitleaks detect` over full history: no leaks.
  - `deny.toml` (cargo-deny permissive-only allow-list) + CI `license-check` job.
  - `tests/repo_hygiene.rs`: guards all wiring (gitleaks/deny config, CI jobs, gitignore
    secret rules, hourly-release script executable + tagging, release-hourlies.md present).
  - Hourly-release automation already shipped (`scripts/release-hourly.sh` +
    `docs/process/release-hourlies.md`); confirmed it cuts a tagged `hourly-<N>` artifact
    and added a guard test.
  - 23 tests green; `./format_code.sh` green. Status → in_review; review gate next.

- **T0+1:05 — BLOCKED by integrator (rebase conflict):**
  Both branch and main added a `pub mod` declaration in `src/lib.rs` at the same line.
  Filed as a trivial additive conflict for reland.

- **T0+2:10 — RELAND by integrator:** rebased onto current main (bb4d112), resolved
  additive conflict in `src/lib.rs` (kept `pub mod licenses;` + `pub mod query;` +
  `pub mod tck;`, sorted). Both review sign-offs (adversarial-reviewer + premortem-analyst)
  preserved from the original review at ef09948.

- **T0+2:15 — LANDED:** Merged to main at c8b20b0. All 58 tests green,
  format_code.sh green. Status: done.
