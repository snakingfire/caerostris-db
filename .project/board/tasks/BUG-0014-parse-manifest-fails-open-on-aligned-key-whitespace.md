---
id: BUG-0014
title: parse_manifest silently drops entries written with aligned-key whitespace (license gate fails open)
type: bug
status: in_review
priority: P2
assignee: implementer (wf_6a2f8faf-da3-7)
epic: EPIC-010
deps: []
rubric_refs: [12]
estimate: S
created: T0+2:45
updated: T0+3:24
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
- Implemented in worktree wf_6a2f8faf-da3-7, branch work/BUG-0014-aligned-key-whitespace.
- adversarial-reviewer signed off (commit 07f6617, T+3:20).
- BLOCKED LANDING T+3:25: premortem-analyst sign-off is missing — checkbox unchecked,
  no verdict appended in PR.md. Integrator returned to author. PR needs premortem-analyst
  to run the pre-mortem loop and append their verdict + tick the checkbox before re-requesting
  landing. See docs/process/adversarial-review-loops.md for the premortem protocol.
- UNBLOCKED T+3:24: premortem-analyst signed off **approve** in worktree wf_6a2f8faf-da3-7
  PR.md (checkbox now ticked). Verified independently: 15/15 `licenses` unit tests +
  2/2 `license_manifest` integration tests green; `./format_code.sh` exit 0; no new dependency
  (Cargo.toml untouched); `git log c3cc51a..main -- src/licenses.rs` empty so it rebases clean.
  Ran an out-of-tree fail-open probe of the fixed `line_value` — every garbled/non-key/embedded-`=`
  line degrades fail-CLOSED. Both review gates now `approve`; ready for the integrator to land.
- INTEGRATOR NOTE (board hygiene, not a code blocker): a DUPLICATE BUG-0014 attempt exists at
  `.worktrees/BUG-0014` (branch `work/BUG-0014-parse-manifest-whitespace`, a `parse_key_value`
  variant that *also* carries an appended pre-mortem). Only ONE may land — this board item names
  worktree `wf_6a2f8faf-da3-7` / branch `work/BUG-0014-aligned-key-whitespace` as canonical; land
  that one and drop/abandon the duplicate to avoid a redundant no-op merge.
