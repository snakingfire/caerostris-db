//! The `caero` binary: a short-name alias for the `caerostris-db` CLI.
//!
//! Identical behaviour to the `caerostris-db` binary; both delegate to
//! [`caerostris_db::cli::run`]. Run `caero --help` for the subcommand list, or
//! `caero demo` for the end-to-end insert → `MATCH` → return demo.

use std::process::ExitCode;

fn main() -> ExitCode {
    caerostris_db::cli::run(std::env::args().skip(1))
}
