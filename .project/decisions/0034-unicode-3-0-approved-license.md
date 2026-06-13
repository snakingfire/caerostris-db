# Decision 0034: Unicode-3.0 is a permissive, approved license (BUG-0023)

**Date:** 2026-06-13 (T0+~3:50)
**Agent:** implementer-wf_156e2b80-bb6-48
**Reversible:** yes (allow-list entry; removable by another edit)
**Status:** recorded local decision — does NOT require steering sign-off (Unicode-3.0
is not copyleft / source-available; the guardrails only gate those on steering).
Board item: `BUG-0023`.

## Decision

`Unicode-3.0` (the Unicode License v3) is an **approved permissive license** for
this project. It is recorded as such in all three layers of the license gate:

- `docs/process/open-source-guardrails.md` §5 "Approved license families" — added.
- `src/licenses.rs` `APPROVED_SPDX` — already present (added with the SPDX
  precedence parser; BUG-0008 era).
- `deny.toml` `[licenses].allow` (root engine workspace) — **added by BUG-0023.**
  `python/deny.toml` already listed it.

The license manifest (`docs/licenses/manifest.toml`) is corrected to record the
**actual** published SPDX of `unicode-ident 1.0.24`:

```
(MIT OR Apache-2.0) AND Unicode-3.0
```

(previously truncated to `MIT OR Apache-2.0`, dropping the `AND Unicode-3.0`
required-compliance conjunct).

## Rationale

- **Unicode-3.0 is permissive.** It is the Unicode Consortium's data license,
  OSI-approved (2024), with no copyleft and no source-availability / non-commercial
  clause. It grants free use, reproduction, and distribution of the Unicode data
  files and software, including in commercial and closed products, with only an
  attribution/notice requirement. It therefore belongs in the same "approved
  permissive families" tier as MIT / Apache-2.0 / BSD, and does **not** fall into
  the copyleft/source-available tier that requires steering sign-off
  (open-source-guardrails.md §5). No steering escalation is required to use it.

- **Why cargo-deny needed the explicit allow.** `unicode-ident` (a transitive
  dependency of `proc-macro2`/`syn`, present in the engine workspace) publishes
  `(MIT OR Apache-2.0) AND Unicode-3.0`. cargo-deny evaluates each crate's *real*
  metadata and, for an `AND`, requires **every** conjunct to be on the allow-list.
  With `Unicode-3.0` absent from the root `deny.toml`, the `license-check` CI job
  was at risk of erroring on that conjunct (relying on tolerant defaults, not an
  explicit allow). License-gate records must be exact (cf. BUG-0008, BUG-0014).

- **Why the manifest was corrected.** The in-repo manifest checker
  (`src/licenses.rs::is_permissive`, parenthesized AND/OR aware since BUG-0008)
  already accepts the full expression, and the truncated form happened to evaluate
  permissive (MIT is approved) so it did not fail open. But the manifest protocol
  mandates recording each crate's *actual* SPDX; the truncation was drift a future
  manifest-vs-reality audit would flag. Recording the real expression closes that
  gap and is now regression-tested.

## Scope check

Spot-checked every crate in `Cargo.lock` and `python/Cargo.lock` whose real
published license carries an `AND` conjunct: **`unicode-ident` is the only one.**
No other manifest-vs-real SPDX drift of this class exists today.

## Evidence

- Verified `unicode-ident 1.0.24` `Cargo.toml` `license = "(MIT OR Apache-2.0) AND
  Unicode-3.0"` in the local registry cache.
- Regression tests in `tests/license_manifest.rs`:
  `unicode_ident_manifest_records_actual_spdx_with_unicode_3_0_conjunct` and
  `root_deny_toml_allows_unicode_3_0` (both fail on pre-BUG-0023 `main`, pass after).

## Alternatives considered

- **Clarify `unicode-ident` in `deny.toml` via `[[licenses.clarify]]` to override
  its SPDX to `MIT OR Apache-2.0`.** Rejected: that *hides* a real conjunct of the
  license rather than honoring it; the guardrails require recording the actual SPDX.
- **Leave `deny.toml` as-is and rely on cargo-deny defaults.** Rejected: relying on
  tolerant defaults instead of an explicit allow is exactly the inexactness this
  bug (and BUG-0008/BUG-0014) exists to eliminate.
