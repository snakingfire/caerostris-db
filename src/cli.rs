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

use crate::storage::{ObjectStore, S3CliStore, StoreError};
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
        Some("minio-demo") => run_minio_demo(),
        Some("s3-ls") => run_s3_ls(args.next().as_deref().unwrap_or("")),
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

/// Build the S3-backed store from the swarm environment, ensuring the bucket
/// exists. Errors are surfaced to the caller as a printed message + failure.
fn open_s3_store() -> Result<S3CliStore, StoreError> {
    let store = S3CliStore::from_swarm_env()?;
    store.ensure_bucket()?;
    Ok(store)
}

/// `caero minio-demo` — the object-storage-native demo: persist a graph to the
/// configured S3/MinIO bucket and answer openCypher `MATCH` queries by reading
/// the objects back. Requires `CAEROSTRIS_S3_ENDPOINT` + `CAEROSTRIS_S3_BUCKET`
/// in the environment (provision via `scripts/env/up.sh` + `bucket.sh`).
fn run_minio_demo() -> ExitCode {
    let mut store = match open_s3_store() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: could not open S3 store: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut stdout = std::io::stdout();
    match demo::run_minio_demo(&mut stdout, &mut store) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: minio-demo failed: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `caero s3-ls [PREFIX]` — list keys in the configured bucket under `PREFIX`
/// (relative to `CAEROSTRIS_S3_PREFIX`), one per line, with object sizes. Used
/// by the demo script to show the bucket contents without an extra CLI
/// dependency.
fn run_s3_ls(prefix: &str) -> ExitCode {
    let store = match open_s3_store() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: could not open S3 store: {e}");
            return ExitCode::FAILURE;
        }
    };
    let keys = match store.list(prefix) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("error: list failed: {e}");
            return ExitCode::FAILURE;
        }
    };
    if keys.is_empty() {
        println!("(no objects)");
        return ExitCode::SUCCESS;
    }
    for key in keys {
        let size = store.get(&key).map(|b| b.len()).unwrap_or(0);
        println!("{key:<20} {size:>6} bytes");
    }
    ExitCode::SUCCESS
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
    println!("    demo                run the in-memory insert -> MATCH -> return demo");
    println!("    minio-demo          persist a graph to S3/MinIO, then query it back");
    println!("    s3-ls [PREFIX]      list objects in the configured S3/MinIO bucket");
    println!("    generate-dataset    emit a synthetic, license-clean property graph");
}
