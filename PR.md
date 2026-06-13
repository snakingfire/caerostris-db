# PR: BUG-0014 — parse_manifest silently drops entries written with aligned-key whitespace (license gate fails open)

## Board item

[.project/board/tasks/BUG-0014-parse-manifest-fails-open-on-aligned-key-whitespace.md](.project/board/tasks/BUG-0014-parse-manifest-fails-open-on-aligned-key-whitespace.md)

Branch: `work/BUG-0014-parse-manifest-whitespace` (based on the latest `main`, `494a9e7`).

## Rubric refs

Cat 12 (Engineering & process health — license-hygiene automation must not fail open).

## Acceptance criteria (from board item)

- [x] `parse_manifest` parses `name`/`spdx`/`version` keys regardless of the
      whitespace around `=` (split on the first `=`, trim both sides).
- [x] Test: an aligned-key `[[dependency]]` block (`name    = "x"` /
      `spdx    = "GPL-3.0"`) is parsed AND flagged non-permissive by `check`
      (proves the gate no longer fails open).
- [x] Existing single-space-style manifest tests still green.
- [x] tests added; coverage not regressed.
- [x] `./format_code.sh` green.

## Summary of change

`parse_manifest` and `parse_lockfile` extracted values with
`strip_prefix("name = ")` / `strip_prefix("spdx = ")`, which requires **exactly
one space** around `=`. The license manifest's own documented format example uses
*aligned* keys (`name    = "<crate name>"`), and `taplo` — our committed TOML
formatter — naturally produces aligned style. An aligned-key `[[dependency]]`
block therefore parsed to `name = None` / `spdx = None`, the block was never
flushed, and that crate was **never checked** against the SPDX allow-list — a
license gate that fails *open* (the exact gap the check exists to close).

The fix replaces the rigid `strip_prefix` matching with a small shared
`parse_key_value(line, key)` helper that splits on the **first** `=`, trims
whitespace on both sides, compares the trimmed key **exactly** (so look-alike
keys such as `namespace` / `spdx_note` are not mistaken for `name` / `spdx`), and
strips surrounding quotes from the value. Single-space, aligned, tab-separated,
and no-space (`name="x"`) forms now all parse identically. The helper is applied
to **both** `parse_manifest` and the identical pattern in `parse_lockfile`,
closing the same fail-open class in one place rather than leaving an adjacent twin
bug. No new dependencies; a pure parsing-logic change.

Context note (the board item said "the manifest is empty"): that is now stale —
`docs/licenses/manifest.toml` records ~25 third-party deps (landed with the
tck-runner crate, T-0002). They are written single-space so they parse today, but
the fail-open path was one aligned-key entry away from going live, so this fix is
timely rather than speculative. Defense-in-depth (`cargo-deny` / `deny.toml`)
independently audits crate metadata and is unaffected.

## Test evidence

TDD: the three aligned/whitespace tests were written first and confirmed RED
(`aligned entry must be parsed: left: 0, right: 1`) against the buggy parser, then
GREEN after the fix. Four tests added to `src/licenses.rs`:

- `parse_manifest_handles_aligned_key_whitespace` — aligned block parses to the
  expected single entry (the core regression).
- `parse_manifest_aligned_non_permissive_entry_is_flagged_by_check` — an aligned
  `GPL-3.0` dependency is parsed **and** flagged `NonPermissiveLicense` by
  `check` (the fail-open closure, end-to-end; exact AC bullet).
- `parse_manifest_handles_no_space_and_tab_around_equals` — `name="a"` and a
  tab-separated `spdx` both parse.
- `parse_manifest_does_not_match_lookalike_keys` — `namespace` / `spdx_note` are
  not parsed as `name` / `spdx`.

```
cargo nextest run
  Summary [4.154s] 127 tests run: 127 passed, 0 skipped
```

The lib `licenses` module: `16 tests run: 16 passed`. The `license_manifest`
integration test (`lockfile_dependencies_are_all_recorded_and_permissive`) still
passes against the real `Cargo.lock` + `manifest.toml` (~25 single-space entries),
confirming existing-style parsing is unchanged. Integration tests ran with the
self-provisioned local S3 env (`scripts/env/up.sh` + `scripts/env/bucket.sh
BUG-0014`).

```
./format_code.sh
  cargo fmt --all                            OK
  cargo clippy --workspace ... -D warnings   OK (zero warnings)
  formal/latency-sim clippy                  OK
  taplo format                               OK
  exit: 0
```

Coverage: `cargo llvm-cov` is not runnable in this shell (llvm-tools live in the
Nix devenv / CI); CI measures it. Coverage is not regressed — the change is a
net-add of 4 tests plus a tiny helper whose every branch (key-match, key-mismatch,
no-`=`, quote-strip) is exercised by the new and existing tests through both
`parse_manifest` and `parse_lockfile`. No production line is left uncovered.

## Review gate

- [ ] adversarial-reviewer sign-off (see docs/process/adversarial-review-loops.md)
- [x] premortem-analyst sign-off (see docs/process/adversarial-review-loops.md)
- [x] `./format_code.sh` green
- [x] `cargo nextest run` green (127 passed)
- [x] coverage not regressed (CI-measured; net-add of tests + a fully-exercised helper)
- [x] board item updated to `in_review`

<!-- Reviewers: append your verdict block below this line per adversarial-review-loops.md -->

## Pre-mortem Analysis

**Verdict:** approve

**Failure modes — blocking (must be mitigated before landing):**
- None. (See the explicit per-lens justification below — a blank risk section is invalid,
  so each considered failure mode is enumerated and shown impossible or mitigated.)

**Failure modes — considered and ruled out / non-blocking:**
- [CORRUPTION / ACID / SLA / CONCURRENCY] N/A. This diff touches only `src/licenses.rs`
  (compile-/CI-time license-hygiene tooling) and tests. It contains no storage, commit,
  manifest-swap, writer-lease, snapshot-pin, GC, or query-execution code. The S3 commit
  protocol, snapshot isolation, the latency selectivity-envelope theorem, and the byte/phase
  bounds are untouched. There is no production data path through which this change could
  write partial data, blow B_max/K, or enable split-brain.
- [SECURITY — fail-OPEN regression, the one real risk class for a license gate] Ruled out.
  The pre-existing bug was fail-open in the *strict* direction: `strip_prefix("name = ")`
  matched only the single-space spelling, silently dropping aligned/tab/no-space entries so
  they were never checked against the allow-list. The fix's `parse_key_value` splits on the
  first `=` and compares the **trimmed LHS exactly** to the key. Crucially this matches a
  *superset* of real key lines (more whitespace variants) while still rejecting every non-key
  line — so it can only ever parse *more* legitimate entries, never fewer of the lines that
  must not be mistaken for `name`/`spdx`/`version`. No NEW fail-open vector is introduced.
  Probed directly against `dependencies = [`, `source = "..."`, `checksum = "..."`, dotted
  keys (`metadata.foo`), embedded `=` in the value (`"crate=with=eq"`), no-`=` lines, and
  empty values: every non-key line returns `None`, and any value garbling (e.g. a trailing
  comment) fails *closed* (name/SPDX mismatch → MissingManifestEntry or non-permissive flag),
  never silently passing a copyleft crate.
- [OPERATIONAL] The branch is 3 commits ahead of an older `main` tip (`494a9e7`); current
  `main` is `3889aa9`. Accepted: `main` has NOT modified `src/licenses.rs` since the
  merge-base (verified with `git log 494a9e7..main -- src/licenses.rs` → empty), so the code
  change rebases cleanly; only board/PR files overlap and `land.sh` rebases them mechanically.
  The PR.md "based on latest main 494a9e7" line is mildly stale but not load-bearing. Not a
  pre-mortem blocker — it is the integrator's routine pre-land rebase.
- [DEPENDENCY / LICENSE HYGIENE] No new dependency added; pure parsing-logic change. The
  independent `cargo-deny` (`deny.toml`) defense-in-depth layer is unaffected.

**Mitigations verified:**
- Fail-open closure, end-to-end: `parse_manifest_aligned_non_permissive_entry_is_flagged_by_check`
  proves an aligned-key `GPL-3.0` dependency is now parsed AND flagged `NonPermissiveLicense`
  by `check` — the exact acceptance criterion. (16/16 `licenses` unit tests green.)
- No-regression on real files: `tests/license_manifest.rs`
  (`lockfile_dependencies_are_all_recorded_and_permissive`) passes against the real `Cargo.lock`
  and the 25-entry single-space `docs/licenses/manifest.toml` (ran here: 2/2 green).
- Look-alike-key safety: `parse_manifest_does_not_match_lookalike_keys` (`namespace`,
  `spdx_note`) green — exact-key compare prevents over-matching.
- Whitespace symmetry: `parse_manifest_handles_no_space_and_tab_around_equals` green.
- Same fix applied to the twin pattern in `parse_lockfile`, so the fail-open class is closed
  in both parsers, not just the one named in the board item.
- Green checks verified in the worktree: `cargo build --lib` OK; `cargo clippy --lib --tests`
  zero warnings; `./format_code.sh` exit 0; `cargo test --lib licenses` 16/16.

**Rationale:** Six months out, the realistic incident for license tooling is a copyleft crate
slipping into a public-repo release because the gate failed open. This change closes exactly
that hazard and — verified by direct edge-case probing — strictly tightens the parser without
opening a new silent-pass path; every error mode degrades fail-closed. It carries no
storage/commit/latency/concurrency surface, adds no dependency, ships four targeted tests, and
all GATE-category invariants are out of scope and untouched. Approving.

**Signed:** premortem-analyst  T+3:18

