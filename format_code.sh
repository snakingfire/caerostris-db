#!/bin/sh
# Run before every commit. Formats + lints Rust and TOML.
set -e

# Format Rust, then lint with clippy (warnings are errors).
# `--workspace` so every workspace member (e.g. tck-runner) is linted, not just
# the root package.
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings

# formal/latency-sim is its own [workspace] and is invisible to the root cargo
# commands above, so we lint it explicitly so local pre-commit catches drift
# before CI does.
cargo fmt --manifest-path formal/latency-sim/Cargo.toml --all
cargo clippy --manifest-path formal/latency-sim/Cargo.toml --all-targets -- -D warnings

# python/ (PyO3 bindings) is likewise its own [workspace]. Lint it WITHOUT the
# extension-module feature (the default) so clippy/fmt do not need an
# interpreter to link against. CI builds the wheel with the feature on.
cargo fmt --manifest-path python/Cargo.toml --all
cargo clippy --manifest-path python/Cargo.toml --all-targets -- -D warnings

# Lint + format-check the Python binding test code if ruff is available (it is
# in the dev environment; CI installs it). Keeps the pytest suite ruff-clean.
if command -v ruff >/dev/null 2>&1; then
    ruff check --config python/ruff.toml python/
    ruff format --config python/ruff.toml python/
else
    echo "Warning: ruff not found; skipping Python lint/format (pip install ruff)."
fi

# Format TOML with taplo if available. Pass explicit paths so taplo does not
# glob-walk the whole tree following symlinks into the .devenv Nix-store links
# (slow enough to read as a hang under tight timeouts).
if command -v taplo >/dev/null 2>&1; then
    taplo format Cargo.toml tck-runner/Cargo.toml python/Cargo.toml rustfmt.toml rust-toolchain.toml
else
    echo "Warning: taplo not found; skipping TOML formatting (run 'direnv reload')."
fi
