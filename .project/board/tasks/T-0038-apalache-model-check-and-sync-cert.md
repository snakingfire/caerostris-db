---
id: T-0038
title: Run Apalache model-check on the commit-protocol TLA+ + sync certification
type: task
status: backlog
priority: P1
assignee:
epic: EPIC-004
deps: [SPIKE-0002, T-0010]
rubric_refs: [11, 1]
estimate: M
created: T0+0:20
updated: T0+0:20
---

## Context

Cat. 11 = 100 requires the TLA+ model from SPIKE-0002 to be model-checked by
Apalache with no invariant violations for the implemented protocol, and kept in
sync with the code (drift = a bug). This task runs Apalache (adding it to the Nix
shell if needed), records the model size + result, and adds the sync-certification
the grader checks each cycle (per `docs/process/formal-verification-policy.md`).
Design-gated on SPIKE-0002 (model exists) and the implementation (T-0010) so sync
can be certified. See `EPIC-004`, `formal/commit-protocol/`.

## Acceptance criteria
- [ ] Apalache available (Nix shell or CI step); the SPIKE-0002 TLA+ model under `formal/commit-protocol/` model-checks with no invariant violations (atomicity, snapshot isolation, fencing/at-most-one-commit-per-version).
- [ ] Model-check command, state count, and result recorded in the ADR or a companion file.
- [ ] Sync certification: a check (CI or documented manual gate) verifies the implemented commit-phase sequence (T-0010) matches the model's spec; its absence downgrades Cat. 11 per policy.
- [ ] Any code/model drift discovered is filed as a BUG, not silently reconciled.
- [ ] tests/checks added; `./format_code.sh` green
- [ ] docs updated (formal-verification policy cross-reference)

## Notes / log
Design-before-code: depends on SPIKE-0002 (the model) and T-0010 (the
implementation to certify against). This is the Cat. 11 commit-protocol evidence.
