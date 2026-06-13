//! Thin CLI entry point over the engine library (`caerostris-db` binary).
//!
//! All dispatch lives in [`caerostris_db::cli`] so this binary and the `caero`
//! alias (`src/bin/caero.rs`) share identical behaviour. Run `caerostris-db
//! --help` for the subcommand list.

use std::process::ExitCode;

fn main() -> ExitCode {
    caerostris_db::cli::run(std::env::args().skip(1))
}
