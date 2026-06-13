---
id: BUG-0014
title: parse_manifest silently drops entries written with aligned-key whitespace (license gate fails open)
type: bug
status: in_review
priority: P2
assignee: implementer-wf_e9fceb87-27c-6
epic: EPIC-010
deps: []
rubric_refs: [12]
estimate: S
created: T0+2:45
updated: T0+3:18
---

## Context

Found during adversarial review of BUG-0008 (SPDX precedence fix). The bug is
**not** in the BUG-0008 diff — it is in the adjacent `parse_manifest` function in
`src/licenses.rs`, which BUG-0008 did not touch — so it is filed separately.

`parse_manifest` extracts keys with `strip_prefix("name = ")` and
`strip_prefix("spdx = ")`, i.e. it requires **exactly one space** around `=`. But
`docs/licenses/manifest.toml`'s own documented format example uses *aligned*
keys:

```
[[dependency]]
name    = "<crate name>"
spdx    = "<SPDX expression>"
```

An entry written in that documented (and taplo-natural) aligned style parses with
`name = None` / `spdx = None`, so the `[[dependency]]` block is never flushed and
that crate is **never checked** against the allow-list.

Reproduced (replica of the parse logic):

```
"name    = \"serde\""  -> name=None spdx=None   // WRONG: should be Some("serde")
"spdx    = \"MIT\""    -> name=None spdx=None   // WRONG: should be Some("MIT")
```

**Why this is dangerous:** the failure is in the *fail-open* direction. A real
dependency recorded in the documented aligned style would silently pass the
license gate without its SPDX expression ever being evaluated — the exact gap the
manifest check exists to close. Today the manifest is empty (zero third-party
deps), so the path is unreached, but this must be fixed **before the first real
dependency is recorded**.

Defense-in-depth note: `cargo-deny` (deny.toml) independently audits crate
license metadata, so a copyleft crate is still caught at that layer. This bug
defeats only the hand-rolled manifest cross-check, not all license enforcement.

## Acceptance criteria
- [ ] `parse_manifest` parses `name`/`spdx`/`version` keys regardless of the
      whitespace around `=` (e.g. split on the first `=`, trim both sides), or
      the manifest format is otherwise made robust to aligned-key style.
- [ ] Test: an aligned-key `[[dependency]]` block
      (`name    = "x"` / `spdx    = "GPL-3.0"`) is parsed AND flagged
      non-permissive by `check` (proves the gate no longer fails open).
- [ ] Existing single-space-style manifest tests still green.
- [ ] tests added; coverage not regressed.
- [ ] `./format_code.sh` green.

## Notes / log
- Filed by adversarial-reviewer during BUG-0008 review. See the BUG-0008 worktree
  PR.md "Adversarial-reviewer verdict" block (non_blocking_notes) for the probe.
- T+3:05 — claimed by implementer-wf_e9fceb87-27c-6 on branch
  `work/BUG-0014-parse-manifest-whitespace`. NOTE: at claim time a concurrent lane
  (wf_6a2f8faf-da3-7, branch `work/BUG-0014-aligned-key-whitespace`) also held a
  `board: claim BUG-0014` commit on its own feature branch (not landed to main; the
  canonical board on main still showed `ready`). Proceeding per explicit dispatch +
  canonical-board-unclaimed; the fix is small and landing is integrator-serialized,
  so whichever PR lands first wins and the other rebases to a no-op / is dropped.
- Context correction: the manifest is no longer empty — `docs/licenses/manifest.toml`
  now records ~25 third-party deps (tck-runner, T-0002). They are written in
  single-space style so they parse today, but the fail-open path is now one
  aligned-key entry away from going live. Fix is timely, not speculative.
- T+3:10 — fix landed on branch (commit e0a639e): shared `parse_key_value` helper
  splits on first `=`, trims both sides, exact key match, strips quotes; applied to
  both `parse_manifest` and `parse_lockfile`. 4 TDD tests added (RED→GREEN). Full
  suite 127/127 green; `./format_code.sh` green. Status -> in_review; dispatching
  adversarial-reviewer + premortem-analyst.
- T+3:18 — premortem-analyst sign-off: **approve** (verdict block appended to PR.md;
  premortem checkbox ticked). Re-verified in the worktree: `licenses` 16/16, the
  `license_manifest` integration test 2/2 against the real Cargo.lock + 25-entry manifest,
  clippy clean, `./format_code.sh` exit 0. Probed `parse_key_value` directly: the fix
  strictly tightens the parser (matches a superset of real key lines, rejects every non-key
  line — `dependencies =`, `source =`, `checksum =`, dotted keys, embedded `=` in value),
  so no new fail-open vector; all error modes degrade fail-closed. No storage/commit/latency/
  concurrency surface touched; no new dependency. Operational note (non-blocking): two other
  branches (`work/BUG-0014-aligned-key-whitespace`, `work/BUG-0014-parse-manifest-silently-...`)
  also did this work — integrator-serialized landing resolves the duplication (first lands;
  others rebase to no-op / drop). main has not touched src/licenses.rs since merge-base 494a9e7,
  so this branch rebases cleanly onto current main (3889aa9).
