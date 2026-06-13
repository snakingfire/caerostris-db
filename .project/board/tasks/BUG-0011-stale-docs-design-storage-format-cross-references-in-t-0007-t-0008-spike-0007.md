---
id: BUG-0011
title: Stale docs/design/storage-format.md cross-references in T-0007, T-0008, SPIKE-0007
type: bug
status: ready
priority: P3
assignee:
epic: EPIC-001
deps: []
rubric_refs: [2, 12]
estimate: S
created: 2026-06-13T20:20:00Z
updated: 2026-06-13T20:20:00Z
---

## Context

Filed by `adversarial-reviewer` during the post-hoc review of BUG-0003 (landed
commit `b14855d`). BUG-0003 repointed the design-spike artifact **commit-targets**
to the canonical `docs/adr/` and `formal/` paths, which closes the GATE-cap risk.
It did **not** sweep stale **cross-reference pointers** to a `docs/design/`
location that does not exist and will not be created:

- `T-0007-columnar-node-property-writer-reader.md` line 23:
  `See \`EPIC-001\`, \`docs/design/storage-format.md\`.`
- `T-0008-adjacency-list-edge-writer-reader.md` line 23:
  `... See \`EPIC-001\`, \`docs/design/storage-format.md\`.`
- `SPIKE-0007-...md` line 44: `(ADR or \`docs/design/\`)`

The storage-format spec is now scoped to land at `docs/adr/0003-storage-format.md`
(corrected SPIKE-0003 AC), and `docs/design/` does not exist in the repo. An
implementer following T-0007/T-0008 will grep for `docs/design/storage-format.md`,
find nothing, and have to hunt for the real spec.

Severity is **lower** than BUG-0003: these are pointers, not artifact commit-targets,
so they do **not** feed the grader-cap mechanism (grader looks at the actual
`docs/adr/`/`formal/` artifacts, not these references). This is stale-doc hygiene
(Cat. 12) plus minor implementer friction on the storage epic (Cat. 2), not a GATE
silently capping. Filed because BUG-0003's own fix already corrected `docs/design/`
→ `docs/adr/` in SPIKE-0001, so leaving three siblings stale is inconsistent.

## Acceptance criteria
- [ ] `T-0007` and `T-0008` cross-references updated from `docs/design/storage-format.md`
      to the canonical `docs/adr/0003-storage-format.md` (or to SPIKE-0003 / EPIC-001
      if a pointer to the spec task is preferred over the not-yet-written ADR file).
- [ ] `SPIKE-0007` AC line 44 `(ADR or \`docs/design/\`)` corrected to drop the
      non-canonical `docs/design/` option (ADR path is `docs/adr/`).
- [ ] Repo-wide grep confirms no remaining `docs/design/` references in
      `.project/board/` or `docs/` except this bug's own description.
- [ ] No code change required; docs/board-text only.
- [ ] `./format_code.sh` green (no Rust touched; trivially green).

## Notes / log
- 2026-06-13T20:20:00Z (adversarial-reviewer): filed as the non-blocking follow-up
  recorded in BUG-0003's review verdict. Same family as BUG-0003/BUG-0010 (docs-path
  hygiene). Low priority — does not block any GATE; fix opportunistically alongside
  the next docs-hygiene pass or when SPIKE-0003's storage ADR lands.
