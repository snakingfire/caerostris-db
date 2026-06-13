---
id: BUG-0023
title: unicode-ident license (Unicode-3.0 conjunct) missing from deny.toml allow-list and misrecorded in manifest
type: bug
status: in_review
priority: P2
assignee: implementer-wf_156e2b80-bb6-48
epic: EPIC-001
deps: []
rubric_refs: [12]
estimate: S
created: T0+3:42
updated: T0+3:55
---

## Context

Found during the adversarial review of T-0030 (PyO3 scaffold). This is a
**pre-existing** defect on `main` (introduced with the `unicode-ident` dependency
via T-0002, not by T-0030), so it was filed separately rather than blocking
T-0030.

`unicode-ident 1.0.24`'s actual published SPDX license is:

```
(MIT OR Apache-2.0) AND Unicode-3.0
```

(verified against the crate's `Cargo.toml` `license` field in the local registry
cache). Two layers of the license gate are inconsistent with this:

1. **`deny.toml`** `[licenses].allow` does **not** include `Unicode-3.0`. Because
   `cargo-deny` evaluates each crate's *real* metadata and requires every `AND`
   conjunct to be allowed, the `license-check` CI job is at risk of erroring on the
   `Unicode-3.0` conjunct. (`src/licenses.rs::APPROVED_SPDX` *does* list
   `Unicode-3.0`, so the in-repo manifest checker would accept the full expression
   — but only if the manifest recorded it, which it does not; see below.)

2. **`docs/licenses/manifest.toml`** records `unicode-ident` as `spdx = "MIT OR
   Apache-2.0"`, dropping the `AND Unicode-3.0` conjunct. This happens to still
   evaluate permissive (MIT is approved), so it does not fail open in this case —
   but the manifest's own protocol says to record the crate's *actual* SPDX, and a
   future audit comparing manifest-vs-reality would flag the drift.

Net effect today: not a fail-open (Unicode-3.0 is a permissive license and is on
the in-repo allow-list), but the `deny.toml`/manifest records are inaccurate and
the `cargo-deny` job may be relying on tolerant defaults rather than an explicit
allow. License-gate records must be exact (cf. BUG-0008, BUG-0014).

## Acceptance criteria
- [ ] `Unicode-3.0` added to `deny.toml` `[licenses].allow` with a note (it is a
      permissive Unicode license; verify it belongs in open-source-guardrails.md §5
      approved families and add it there + to a recorded decision if not already).
- [ ] `docs/licenses/manifest.toml` `unicode-ident` entry records the *actual* SPDX
      `(MIT OR Apache-2.0) AND Unicode-3.0` (the in-repo parser already handles
      parenthesized AND/OR — confirm the entry still passes `is_permissive`).
- [ ] `cargo deny check licenses` passes (run it, paste output) — confirms the gap
      was real or proves it was already tolerated.
- [ ] `tests/license_manifest.rs` still green.
- [ ] No other crate in `Cargo.lock` has the same manifest-vs-real SPDX drift (spot
      check the crates whose real license carries an `AND` conjunct).

## Notes / log
- **T0+3:42 — filed by adversarial-reviewer** during T-0030 review. Verified
  `unicode-ident 1.0.24` real license `(MIT OR Apache-2.0) AND Unicode-3.0` vs.
  manifest `MIT OR Apache-2.0` and absent `Unicode-3.0` in `deny.toml`. Pre-existing
  on `main` (T-0002 era); not introduced by T-0030.
- **T0+3:55 — implementer-wf_156e2b80-bb6-48** TDD-first fix. Branch
  `work/BUG-0023-unicode-3-0-deny-manifest`, PR worktree `.worktrees/BUG-0023`.
  Confirmed the gap was REAL: `cargo deny check licenses` (v0.19.8) FAILS on the
  pre-fix root allow-list (`error[rejected]: license is not explicitly allowed` —
  `Unicode-3.0`, `unicode-ident v1.0.24`); passes (`licenses ok`) after adding
  `Unicode-3.0`. Corrected manifest SPDX, documented Unicode-3.0 (+Zlib) in
  guardrails §5, recorded Decision 0034. Spot-check: `unicode-ident` is the only
  crate in either lockfile with an `AND`-conjunct real license. 233/233 tests green;
  2 new regression tests. Set `in_review`.
