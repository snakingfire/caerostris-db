---
id: T-NNNN
title: <one-line imperative title>
type: task            # epic | story | task | spike | bug
status: backlog       # backlog | ready | in_progress | in_review | blocked | done | dropped
priority: P2          # P0 | P1 | P2 | P3
assignee:             # agent run-id/label; empty if unclaimed
epic:                 # parent epic id (omit for epics)
deps: []              # ids that must be `done` before this becomes `ready`
rubric_refs: []       # master-rubric category numbers this advances
estimate: M           # S | M | L  (prefer S — split L)
created:              # T+ marker or ISO
updated:              # T+ marker or ISO
---

## Context
<Why this exists. Link the spec / ADR / decision / parent epic.>

## Acceptance criteria
- [ ] <concrete, testable outcome — the reviewer checks these>
- [ ] tests added (unit/integration/property/TCK as appropriate); coverage not regressed
- [ ] docs / ADR / CLAUDE.md updated if behaviour or architecture changed
- [ ] `./format_code.sh` green

## Notes / log
<Append-only: claims, decisions, links to the PR worktree, blockers.>
