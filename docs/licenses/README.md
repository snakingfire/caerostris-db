# License hygiene — caerostris-db

> Rubric anchor: **Cat. 12** (engineering & process health). Policy lives in
> [`../process/open-source-guardrails.md`](../process/open-source-guardrails.md)
> §5–§6. This directory is the committed evidence that every dependency is
> license-clean.

## What lives here

| File | Purpose |
|------|---------|
| [`manifest.toml`](manifest.toml) | The dependency → SPDX license ledger. Every third-party crate in `Cargo.lock` must have an entry. |

## The two-layer check

License hygiene is enforced by two independent, defense-in-depth layers so a
non-permissive or unrecorded dependency cannot land silently:

1. **In-repo manifest check** (`tests/license_manifest.rs`, logic in
   [`../../src/licenses.rs`](../../src/licenses.rs)). Runs under
   `cargo nextest run` / `cargo test` and in CI. It:
   - parses `Cargo.lock` for every resolved dependency (skipping our own crate),
   - fails if any dependency is **missing** from `manifest.toml`,
   - fails if any recorded SPDX id is **not in the approved allow-list**.

   This needs no external tools, so it works in every environment.

2. **`cargo-deny`** ([`../../deny.toml`](../../deny.toml)) in CI. It reads the
   real license metadata from each crate (not just our hand-written manifest)
   and enforces a permissive-only allow-list. This catches a manifest entry that
   *claims* a permissive license a crate does not actually carry.

The approved allow-list (MIT, Apache-2.0, BSD-2/3-Clause, ISC, MPL-2.0, CC0-1.0,
Unlicense, Zlib) mirrors `open-source-guardrails.md`. Copyleft / source-available
licenses (GPL, LGPL, AGPL, SSPL, BUSL, CC-BY-NC) require a recorded steering
decision in `.project/decisions/` **before** the dependency is added.

## Adding a dependency

```bash
# 1. Find the SPDX id of the crate you want to add.
cargo license --avoid-build-deps        # or read the crate's `license` field

# 2. If permissive: add it to Cargo.toml, then record it in manifest.toml.
#    If copyleft/source-available: file a steering decision FIRST.

# 3. Update the manifest:
#    [[dependency]]
#    name    = "serde"
#    version = "1.0.210"
#    spdx    = "MIT OR Apache-2.0"
#    note    = "Permissive; serialization core."

# 4. Verify both layers are green:
cargo nextest run license_manifest      # in-repo check
cargo deny check licenses               # metadata cross-check (CI installs it)
```

If you skip step 3, the manifest check fails CI with an actionable message
naming the missing crate — that is the guard working as designed.
