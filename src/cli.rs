//! The command-line dispatch shared by the `caerostris-db` and `caero`
//! binaries.
//!
//! Both binary entry points (`src/main.rs` and `src/bin/caero.rs`) are thin
//! wrappers that call [`run`] with the process arguments; keeping the logic in
//! the library means the two binary names stay byte-for-byte identical in
//! behaviour and the dispatch is unit-testable.
//!
//! Subcommands:
//! - `demo` — run the end-to-end insert → `MATCH` → return demo
//!   ([`crate::demo`]). No flags; prints labelled results to stdout.
//! - `generate-dataset` — emit a synthetic, license-clean graph
//!   ([`crate::dataset`]).

use std::process::ExitCode;

use crate::{dataset, demo, version};

const GENERATE_USAGE: &str = "\
caerostris-db generate-dataset — emit a synthetic, license-clean property graph

USAGE:
    caerostris-db generate-dataset [--nodes N] [--edges M] [--seed S] [--zipf E] [--out PATH]

OPTIONS:
    --nodes N    node count (default 1000000)
    --edges M    edge count (default 10000000)
    --seed  S    PRNG seed; identical seed ⇒ identical graph (default 0)
    --zipf  E    Zipf exponent for the power-law tail, > 0 (default 1.0)
    --out   PATH write JSONL here (default: stdout)

The output is deterministic and carries no third-party data or licence.";

/// Run the CLI given the program arguments (already excluding `argv[0]`).
#[must_use]
pub fn run<I, S>(args: I) -> ExitCode
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);
    match args.next().as_deref() {
        Some("demo") => run_demo(),
        Some("generate-dataset") => {
            let rest: Vec<String> = args.collect();
            if rest.iter().any(|a| a == "-h" || a == "--help") {
                println!("{GENERATE_USAGE}");
                return ExitCode::SUCCESS;
            }
            run_generate(rest)
        }
        Some("--help" | "-h") | None => {
            print_top_level_help();
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("error: unknown subcommand {other:?}");
            print_top_level_help();
            ExitCode::FAILURE
        }
    }
}

fn run_demo() -> ExitCode {
    let mut stdout = std::io::stdout();
    match demo::run_demo(&mut stdout) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: demo failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_generate(rest: Vec<String>) -> ExitCode {
    let parsed = match dataset::parse_args(rest) {
        Ok(p) => p,
        Err(msg) => {
            eprintln!("error: {msg}");
            eprintln!("{GENERATE_USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let mut stderr = std::io::stderr();
    match dataset::cli::run(&parsed, &mut stderr) {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: failed to generate dataset: {e}");
            ExitCode::FAILURE
        }
    }
}

fn print_top_level_help() {
    println!("caerostris-db {}", version());
    println!("graph database engine on durable object storage");
    println!();
    println!("USAGE:");
    println!("    caero <SUBCOMMAND>          (alias: caerostris-db)");
    println!();
    println!("SUBCOMMANDS:");
    println!("    demo                run the end-to-end insert -> MATCH -> return demo");
    println!("    generate-dataset    emit a synthetic, license-clean property graph");
}
