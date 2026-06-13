# caerostris-db — agent guide

A graph database engine built from the ground up on commodity durable object
storage (S3-like), inspired by DuckDB. Named for *Caerostris darwini*, the
spider with the toughest silk known.

**Status:** toolchain scaffolding only. Product requirements and the engine
architecture are still TBD — do not invent them. When real requirements arrive,
brainstorm → spec (`docs/superpowers/specs/`) → plan → implement.

## Dev workflow

This is a Rust project. Tooling comes from a Nix `devenv` shell (`flake.nix`),
auto-loaded via direnv (`direnv allow`); non-Nix users get the same stable
toolchain via `rust-toolchain.toml` + rustup.

```bash
cargo build
cargo test                                   # unit tests + doctests
cargo nextest run                            # faster, in the Nix shell
cargo clippy --all-targets -- -D warnings
./format_code.sh                             # ALWAYS run before committing
```

## Conventions

- **Run `./format_code.sh` before every commit** (cargo fmt + clippy -D warnings
  + taplo). CI enforces fmt, clippy, and tests.
- **Clippy warnings are errors.** Keep the tree warning-clean.
- **Never commit secrets or data.** gitleaks runs in pre-commit; `.env*`, keys,
  and `/target` are gitignored.
- **`Cargo.lock` is committed** (this crate ships a binary).
- **Never use destructive git operations** (`reset --hard`, `push --force`,
  branch deletion) without explicit authorization for that exact action.
- Single crate for now (`lib.rs` core + thin `main.rs`). Promote to a Cargo
  workspace when the engine splits into multiple crates.
