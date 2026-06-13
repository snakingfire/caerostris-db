//! Thin CLI entry point over the engine library.
//!
//! Subcommands:
//! - `generate-dataset` — write a synthetic, license-clean graph (see
//!   [`caerostris_db::dataset`]). Run with `--help` for flags.

use std::process::ExitCode;

use caerostris_db::dataset;
use caerostris_db::version;

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

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
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
    println!("    caerostris-db <SUBCOMMAND>");
    println!();
    println!("SUBCOMMANDS:");
    println!("    generate-dataset    emit a synthetic, license-clean property graph");
}
