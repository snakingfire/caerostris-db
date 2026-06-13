---
id: BUG-0011
title: Stale docs/design/storage-format.md cross-references in T-0007, T-0008, SPIKE-0007
type: bug
status: in_review
priority: P3
assignee: implementer-wf_86b0c2e8-f29-13
epic: EPIC-001
deps: []
rubric_refs: [2, 12]
estimate: S
created: 2026-06-13T20:20:00Z
updated: 2026-06-13T21:06:00Z
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
- [x] `T-0007` and `T-0008` cross-references updated from `docs/design/storage-format.md`
      to point at the storage-format spec **owned by `SPIKE-0003`** (which lands under
      `docs/adr/`). Chose the spec-task pointer over a hard-coded `docs/adr/0003-…` filename
      because ADR `0003` is already taken by `0003-server-mode-network-protocol.md` (the very
      numbering-collision tracked by BUG-0010) — pinning a contested filename would just
      re-introduce a stale pointer. `EPIC-001` is retained alongside.
- [x] `SPIKE-0007` AC line 44 `(ADR or \`docs/design/\`)` corrected to drop the
      non-canonical `docs/design/` option → now reads "an ADR under `docs/adr/`".
- [x] Repo-wide grep confirms no remaining `docs/design/` references in
      `.project/board/` or `docs/` except this bug's own description and BUG-0003
      (the parent docs-path bug + its review verdict, which *record* the defect — the same
      "historical record" exception BUG-0003 applied to SPIKE-0002 line 38).
- [x] No engine/`src` code changed; the diff is board-text **plus** a docs-hygiene
      regression guard added to the existing Cat. 12 test file
      (`tests/repo_hygiene.rs::no_stale_docs_design_references`) so the AC-#3 grep is
      enforced by `cargo test` / CI and recurrence is prevented. Implemented as a Rust
      test (not a standalone script) to match the repo's existing `repo_hygiene.rs`
      hygiene-guard pattern and run in the already-wired CI `test` job — directly serving
      the Cat. 12 rubric_ref, not just a one-time text edit.
- [x] `./format_code.sh` green (fmt + clippy `-D warnings` + taplo; the only Rust
      touched is the new test, which is warning-clean).

## Notes / log
- 2026-06-13T20:20:00Z (adversarial-reviewer): filed as the non-blocking follow-up
  recorded in BUG-0003's review verdict. Same family as BUG-0003/BUG-0010 (docs-path
  hygiene). Low priority — does not block any GATE; fix opportunistically alongside
  the next docs-hygiene pass or when SPIKE-0003's storage ADR lands.
- 2026-06-13T20:38:00Z (implementer-wf_86b0c2e8-f29-13): claimed; branch
  `work/BUG-0011-stale-docs-design-storage-format-md-cross-referenc` off latest main
  (`2b87e70`). TDD-first: added a guard test
  `tests/repo_hygiene.rs::no_stale_docs_design_references` (RED with stash — found the
  three actionable stale refs in T-0007/T-0008/SPIKE-0007), repointed all three to the
  `SPIKE-0003`-owned spec / `docs/adr/`, re-ran the test (GREEN). Guard lives in the
  existing Cat. 12 hygiene file and runs in the wired CI `test` job so AC-#3 stays
  enforced. Raw grep confirms only BUG-0011 and BUG-0003 retain `docs/design/` (both
  legitimate defect records — the "historical record" exception). `./format_code.sh`
  green; full `cargo nextest run` green (57 passed, 0 skipped). PR opened; → in_review.
- 2026-06-13T21:03:00Z (adversarial-reviewer): **APPROVE** (verdict appended to PR.md).
  Attacks tried and survived: vacuous-test (verified RED on injected ref), allowlist
  bypass, walker symlink loop, stale-ref hiding outside searched dirs, wrong canonical
  target (confirmed ADR 0003 is server-mode, so the SPIKE-0003 spec pointer is correct),
  secrets/deps/unsafe. Re-ran `./format_code.sh` (exit 0) + full `cargo test` (all green)
  locally. No blocking findings; 3 non-blocking notes (PR.md churn from pre-existing
  BUG-0013, a minor commit-attribution inaccuracy in BUG-0013's body, AC #4 re-wording).
  Awaiting premortem sign-off before the integrator lands.
- 2026-06-13T21:06:00Z (premortem-analyst): **APPROVE** (verdict appended to PR.md;
  premortem box ticked). No code path in this diff can corrupt data, regress the SLA,
  or split-brain — no `src`/storage/commit/lease/cache code is touched; the only Rust
  is a read-only, std-only CI guard that fails safe. Independently re-verified the guard
  is RED-capable (injected a `docs/design/` ref into a non-allowlisted doc → test FAILED,
  reverted clean), `./format_code.sh` exit 0, and full `cargo test` green. Residual risks
  are all P3 operational/accuracy notes (PR.md churn → BUG-0013; allowlist-by-file;
  walker error-swallow) — accepted/already filed. Both gates green; clear to land.
