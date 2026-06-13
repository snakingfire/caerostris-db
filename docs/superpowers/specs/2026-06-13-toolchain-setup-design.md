# caerostris-db — Toolchain & Repo Init (Design)

**Date:** 2026-06-13
**Status:** Approved (Option A)

## Goal

Initialize the `caerostris-db` repository and its Rust toolchain so the project
can "hit the ground running" once requirements land. The repo is a public,
open-source hackathon project. The eventual product is a **graph database engine
written from the ground up, backed by commodity durable object storage (e.g.
S3)**, taking inspiration from systems like DuckDB. No product code is in scope
yet — only a validated, reproducible toolchain and the repo scaffolding around it.

Named for *Caerostris darwini* (Darwin's bark spider), whose dragline silk is the
toughest known biological material.

## Decisions

- **License:** MIT (single file, zero friction — right call for a hackathon).
- **Nix:** `devenv` + `flake-parts`, mirroring the sibling `gtm-eng` repo's
  pattern (Option A). Swap Python/uv for Rust. This machine has Nix + direnv.
- **Non-Nix path:** `rust-toolchain.toml` (rustup) + standard `cargo`, so
  contributors without Nix get an equivalent stable toolchain.
- **Crate layout:** single crate (`lib.rs` engine core + thin `main.rs` binary).
  Grow into a Cargo workspace when requirements justify multiple crates.

## Components

### Reproducible dev environment (Nix)
- `flake.nix` — `devenv.shells.default` with `languages.rust` (channel
  `stable`: rustc, cargo, clippy, rustfmt, rust-analyzer) plus `cargo-nextest`,
  `taplo`, `gitleaks`, `pre-commit`, `jq`.
- `.envrc` — direnv auto-activation (mirrors gtm-eng, with the devenv-root
  override).

### Toolchain for everyone else (no Nix)
- `rust-toolchain.toml` — pins the stable channel + `rustfmt`, `clippy`,
  `rust-analyzer` for rustup users and CI.

### Crate scaffold
- `Cargo.toml` — package metadata, edition 2024, MIT, MSRV 1.85.
- `src/lib.rs` — minimal public surface (`version()`) with a unit test and a
  doctest, to exercise build/test/doctest/clippy/fmt.
- `src/main.rs` — thin binary over the lib.
- `rustfmt.toml` — formatting config.

### Quality gates
- `.pre-commit-config.yaml` — `cargo fmt`, `cargo clippy -D warnings`, `taplo`,
  `gitleaks`.
- `format_code.sh` — run-before-commit formatter/linter (mirrors gtm-eng).
- `.github/workflows/ci.yml` — two jobs: (1) cargo `fmt --check` + `clippy -D
  warnings` + `test` on the rustup toolchain; (2) `nix develop` to prove the
  flake's dev shell still builds.

### Docs / onboarding
- `README.md` — what it is + setup for both Nix and non-Nix users.
- `CLAUDE.md` — concise standing instructions for Claude Code sessions; notes
  that product requirements are still TBD.
- `AGENTS.md` — one-line pointer to `CLAUDE.md` for tool-agnostic agents.
- `LICENSE` — MIT.
- `.gitignore` — Rust (`/target`) + Nix/direnv + secrets + editor cruft.
  `Cargo.lock` is committed (this ships a binary).

## Validation (definition of done)

Inside the Nix dev shell, all of the following pass:
`rustc --version`, `cargo --version`, `cargo fmt --all --check`,
`cargo clippy --all-targets -- -D warnings`, and the test suite
(`cargo nextest run` / `cargo test`).

## Out of scope

Any database/graph/storage logic. The engine design is a separate spec that
follows once requirements are clarified.
