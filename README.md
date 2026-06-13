# caerostris-db

A graph database engine, written from the ground up to run on **commodity
durable object storage** (e.g. S3), taking inspiration from systems like
[DuckDB](https://duckdb.org/).

Named for *[Caerostris darwini](https://en.wikipedia.org/wiki/Caerostris_darwini)*
(Darwin's bark spider), which spins the strongest webs of any spider in the
world — its dragline silk is the toughest known biological material.

> **Status:** early scaffolding. The toolchain is set up and validated; the
> engine's product requirements and architecture are still being defined.

## Getting started

You need a Rust toolchain. There are two supported paths.

### With Nix (recommended — reproducible)

This repo ships a [`devenv`](https://devenv.sh/) flake. With Nix and
[direnv](https://direnv.net/) installed:

```bash
direnv allow      # activates the dev shell: rust (stable), clippy, rustfmt,
                  # rust-analyzer, cargo-nextest, taplo, gitleaks, pre-commit
```

Without direnv, enter the shell manually:

```bash
nix develop --impure
```

### Without Nix

Install [rustup](https://rustup.rs/). The pinned toolchain in
`rust-toolchain.toml` (stable + `rustfmt`, `clippy`, `rust-analyzer`) is
installed automatically the first time you run `cargo`:

```bash
cargo build
```

## Common commands

```bash
cargo build                 # build
cargo run                   # run the (scaffold) binary
cargo test                  # run unit tests + doctests
cargo nextest run           # faster test runner (in the Nix shell)
cargo clippy --all-targets -- -D warnings   # lint
cargo fmt --all             # format
./format_code.sh            # format + lint everything (run before committing)
```

Optionally install the git hooks (gitleaks, fmt, clippy, taplo):

```bash
pre-commit install
```

## Project layout

```
flake.nix              # Nix devenv dev shell (Rust toolchain + dev tools)
.envrc                 # direnv auto-activation of the dev shell
rust-toolchain.toml    # toolchain pin for rustup / non-Nix users + CI
Cargo.toml             # crate manifest (single crate: lib + thin binary)
rustfmt.toml           # formatting config
src/
  lib.rs               # engine core (scaffold)
  main.rs              # thin binary entry point
.github/workflows/     # CI: fmt + clippy + test, and a Nix dev-shell build
docs/                  # design specs and project docs
```

## License

[MIT](./LICENSE).
