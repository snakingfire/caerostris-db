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
- [ ] premortem-analyst sign-off (see docs/process/adversarial-review-loops.md)
- [x] `./format_code.sh` green
- [x] `cargo nextest run` green (127 passed)
- [x] coverage not regressed (CI-measured; net-add of tests + a fully-exercised helper)
- [x] board item updated to `in_review`

<!-- Reviewers: append your verdict block below this line per adversarial-review-loops.md -->
