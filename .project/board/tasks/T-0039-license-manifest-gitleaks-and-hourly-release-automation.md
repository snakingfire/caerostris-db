---
id: T-0039
title: License manifest, gitleaks pre-commit, and hourly-release automation
type: task
status: ready
priority: P2
assignee:
epic: EPIC-010
deps: []
rubric_refs: [12]
estimate: S
created: T0+0:20
updated: T0+0:20
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
