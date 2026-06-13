#!/bin/sh
# Run before every commit. Formats + lints Rust and TOML.
set -e

# Format Rust, then lint with clippy (warnings are errors).
# `--workspace` so every workspace member (e.g. tck-runner) is linted, not just
# the root package.
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Format TOML with taplo if available. Pass explicit paths so taplo does not
# glob-walk the whole tree following symlinks into the .devenv Nix-store links
# (slow enough to read as a hang under tight timeouts).
if command -v taplo >/dev/null 2>&1; then
    taplo format Cargo.toml tck-runner/Cargo.toml rustfmt.toml rust-toolchain.toml
else
    echo "Warning: taplo not found; skipping TOML formatting (run 'direnv reload')."
fi
