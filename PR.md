# PR: BUG-0011 — Stale docs/design/storage-format.md cross-references in T-0007, T-0008, SPIKE-0007

## Board item

[.project/board/tasks/BUG-0011-stale-docs-design-storage-format-cross-references-in-t-0007-t-0008-spike-0007.md](.project/board/tasks/BUG-0011-stale-docs-design-storage-format-cross-references-in-t-0007-t-0008-spike-0007.md)

## Rubric refs

Cat 12 (engineering & process health — docs/board hygiene), Cat 2 (storage format
& S3 commit protocol — removes implementer friction on the storage epic).

## Acceptance criteria (from board item)

- [x] `T-0007` and `T-0008` cross-references updated from `docs/design/storage-format.md`
      to point at the storage-format spec owned by `SPIKE-0003` (lands under `docs/adr/`).
- [x] `SPIKE-0007` AC line 44 `(ADR or `docs/design/`)` corrected to drop the
      non-canonical `docs/design/` option.
- [x] Repo-wide grep confirms no remaining `docs/design/` references in `.project/board/`
      or `docs/` except this bug's own description and BUG-0003 (the parent docs-path bug
      + its review verdict, which record the defect — same "historical record" exception
      BUG-0003 applied to SPIKE-0002).
- [x] No engine/`src` code changed; diff is board-text plus a docs-hygiene regression
      guard in the existing Cat. 12 test file.
- [x] `./format_code.sh` green.

## Summary of change

Sweeps the three stale `docs/design/storage-format.md` cross-reference pointers that
BUG-0003 left behind when it repointed the design-spike *commit-targets* (but not the
*pointers*) to canonical paths. `docs/design/` does not exist and will not be created;
an implementer following T-0007/T-0008 would grep for it and find nothing.

- **T-0007** and **T-0008** Context lines now point at the storage-format spec **owned by
  `SPIKE-0003`** (which lands under `docs/adr/`), keeping the existing `EPIC-001` pointer.
  I deliberately chose the spec-*task* pointer over a hard-coded `docs/adr/0003-…` filename
  because ADR number `0003` is already taken by `0003-server-mode-network-protocol.md` (the
  exact numbering collision tracked by BUG-0010) — pinning a contested filename would just
  re-introduce a stale pointer.
- **SPIKE-0007** AC line 44 `(ADR or `docs/design/`)` now reads "an ADR under `docs/adr/`".
- Added a TDD regression guard `tests/repo_hygiene.rs::no_stale_docs_design_references`
  that fails CI if any non-allowlisted board/docs file references `docs/design/`. It is a
  Rust test (not a standalone script) to match the repo's existing `repo_hygiene.rs`
  Cat. 12 hygiene-guard pattern and run in the already-wired CI `test` job. The allowlist
  is the two files whose subject *is* the defect: BUG-0011 (this bug) and BUG-0003 (parent
  + verdict), matched by filename prefix so a slug rename cannot silently re-open the gap.

No engine/`src` code touched; the only Rust is the new test. (Note: filed BUG-0013 for a
pre-existing, out-of-scope issue spotted here — a stray `PR.md` was committed to `main` by
T-0039 commit `f67868b`; `PR.md` should be gitignored, not tracked. This PR replaces that
stray file's contents with the BUG-0011 description; the gitignore/untrack fix is BUG-0013.)

## Test evidence

TDD RED→GREEN, captured with the doc fixes stashed/un-stashed:

- **RED** (stale refs present): `no_stale_docs_design_references` FAILED, listing exactly
  the three actionable refs (T-0007:23, T-0008:23, SPIKE-0007:44) and correctly excluding
  BUG-0011/BUG-0003 via the allowlist.
- **GREEN** (after fixes): test passes.

```
$ cargo nextest run
     Summary [   1.254s] 57 tests run: 57 passed, 0 skipped
        PASS  caerostris-db::repo_hygiene no_stale_docs_design_references
```

```
$ cargo test --test repo_hygiene
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

```
$ ./format_code.sh        # cargo fmt + cargo clippy --all-targets -- -D warnings + taplo
$ cargo clippy --all-targets --all-features -- -D warnings
    Finished `dev` profile     # zero warnings
```

Raw verification of AC #3 (no allowlist applied):

```
$ grep -rln 'docs/design/' .project/board docs | grep -vE 'BUG-0011-|BUG-0003-'
(no output — only BUG-0011 and BUG-0003 retain the path, both as defect records)
```

Coverage: this PR adds a passing test and changes no `src/` code, so engine line coverage
is unchanged (not regressed); the new test increases the hygiene-guard surface.

## Review gate

- [ ] adversarial-reviewer sign-off (see docs/process/adversarial-review-loops.md)
- [ ] premortem-analyst sign-off (see docs/process/adversarial-review-loops.md)
- [ ] `./format_code.sh` green
- [ ] `cargo nextest run` green (or `cargo test` outside Nix shell)
- [ ] coverage not regressed
- [ ] board item updated to `in_review`

<!-- Reviewers: append your verdict block below this line per adversarial-review-loops.md -->
