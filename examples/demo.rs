//! End-to-end demo runnable with `cargo run --example demo`.
//!
//! Builds a tiny graph in memory, inserts `(:Person {name: 'Alice', age: 30})`,
//! `(:Person {name: 'Bob'})`, and `Alice-[:KNOWS]->Bob`, then runs two
//! openCypher `MATCH` queries and prints the rows — proving inserted data is
//! returned by a query. The same routine backs the `caero demo` subcommand.

use std::process::ExitCode;

use caerostris_db::demo;

fn main() -> ExitCode {
    let mut stdout = std::io::stdout();
    match demo::run_demo(&mut stdout) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: demo failed: {e}");
            ExitCode::FAILURE
        }
    }
}
