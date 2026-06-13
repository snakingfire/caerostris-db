//! A tiny argument parser for the `generate-dataset` CLI subcommand.
//!
//! Deliberately dependency-free (no `clap`): the surface is a handful of
//! `--key value` flags, so a hand-rolled parser keeps the binary lean and the
//! logic unit-testable without spinning up a process. `main.rs` is a thin
//! dispatcher over [`parse_args`] + [`run`].
//!
//! Usage:
//!
//! ```text
//! caerostris-db generate-dataset [--nodes N] [--edges M] [--seed S]
//!                                [--zipf E] [--out PATH]
//! ```
//!
//! With no `--out`, the JSONL stream is written to stdout (so it can be piped).
//! Defaults are the headline 1M-node / 10M-edge graph.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;

use super::{GenConfig, GenStats, Generator, write_jsonl};

/// A parsed `generate-dataset` invocation.
///
/// [`Default`] is the headline 1M/10M graph to stdout (mirrors
/// [`GenConfig::default`] with no output path).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GenerateArgs {
    /// The generation config (size, seed, shape).
    pub config: GenConfig,
    /// Output path, or `None` for stdout.
    pub out: Option<PathBuf>,
}

/// Parse the arguments *after* the `generate-dataset` subcommand token.
///
/// # Errors
///
/// Returns a human-readable message on an unknown flag, a missing value, or a
/// value that fails to parse.
pub fn parse_args<I, S>(args: I) -> Result<GenerateArgs, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut parsed = GenerateArgs::default();
    let mut it = args.into_iter().map(Into::into);
    while let Some(flag) = it.next() {
        let want_value = |it: &mut dyn Iterator<Item = String>| {
            it.next()
                .ok_or_else(|| format!("flag {flag} requires a value"))
        };
        match flag.as_str() {
            "--nodes" => {
                parsed.config.node_count = parse_u64(&flag, &want_value(&mut it)?)?;
            }
            "--edges" => {
                parsed.config.edge_count = parse_u64(&flag, &want_value(&mut it)?)?;
            }
            "--seed" => {
                parsed.config.seed = parse_u64(&flag, &want_value(&mut it)?)?;
            }
            "--zipf" => {
                let v = want_value(&mut it)?;
                let e: f64 = v
                    .parse()
                    .map_err(|_| format!("--zipf expects a number, got {v:?}"))?;
                // Reject zero, negative, and non-finite (NaN/inf) exponents.
                if e <= 0.0 || !e.is_finite() {
                    return Err(format!("--zipf must be a finite value > 0, got {e}"));
                }
                parsed.config.zipf_exponent = e;
            }
            "--out" => {
                parsed.out = Some(PathBuf::from(want_value(&mut it)?));
            }
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    Ok(parsed)
}

fn parse_u64(flag: &str, v: &str) -> Result<u64, String> {
    v.parse()
        .map_err(|_| format!("{flag} expects a non-negative integer, got {v:?}"))
}

/// Run a parsed `generate-dataset` invocation, writing a progress line to
/// `status` (e.g. stderr) and the graph to the configured destination.
///
/// # Errors
///
/// Returns any I/O error from opening the output file or writing the stream.
pub fn run<W: Write>(args: &GenerateArgs, status: &mut W) -> io::Result<GenStats> {
    let generator = Generator::new(args.config.clone());
    let stats = match &args.out {
        Some(path) => {
            let file = File::create(path)?;
            let mut w = BufWriter::new(file);
            let s = write_jsonl(&generator, &mut w)?;
            w.flush()?;
            writeln!(
                status,
                "wrote {} nodes, {} edges ({} bytes) to {}",
                s.nodes_written,
                s.edges_written,
                s.bytes_written,
                path.display()
            )?;
            s
        }
        None => {
            let stdout = io::stdout();
            let mut w = BufWriter::new(stdout.lock());
            let s = write_jsonl(&generator, &mut w)?;
            w.flush()?;
            s
        }
    };
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_headline_size_to_stdout() {
        let args = parse_args(Vec::<String>::new()).expect("parse");
        assert_eq!(args, GenerateArgs::default());
        assert_eq!(args.config.node_count, 1_000_000);
        assert_eq!(args.config.edge_count, 10_000_000);
        assert!(args.out.is_none());
    }

    #[test]
    fn parses_all_flags() {
        let args = parse_args([
            "--nodes",
            "100",
            "--edges",
            "250",
            "--seed",
            "7",
            "--zipf",
            "1.5",
            "--out",
            "/tmp/g.jsonl",
        ])
        .expect("parse");
        assert_eq!(args.config.node_count, 100);
        assert_eq!(args.config.edge_count, 250);
        assert_eq!(args.config.seed, 7);
        assert!((args.config.zipf_exponent - 1.5).abs() < 1e-12);
        assert_eq!(args.out, Some(PathBuf::from("/tmp/g.jsonl")));
    }

    #[test]
    fn unknown_flag_is_an_error() {
        let err = parse_args(["--bogus", "1"]).unwrap_err();
        assert!(err.contains("unknown flag"), "got: {err}");
    }

    #[test]
    fn missing_value_is_an_error() {
        let err = parse_args(["--nodes"]).unwrap_err();
        assert!(err.contains("requires a value"), "got: {err}");
    }

    #[test]
    fn non_integer_value_is_an_error() {
        let err = parse_args(["--nodes", "lots"]).unwrap_err();
        assert!(err.contains("non-negative integer"), "got: {err}");
    }

    #[test]
    fn non_positive_zipf_is_an_error() {
        let err = parse_args(["--zipf", "0"]).unwrap_err();
        assert!(err.contains("> 0"), "got: {err}");
    }

    #[test]
    fn non_finite_zipf_is_an_error() {
        for bad in ["nan", "inf", "-1.5"] {
            let err = parse_args(["--zipf", bad]).unwrap_err();
            assert!(err.contains("> 0"), "for {bad:?} got: {err}");
        }
    }

    #[test]
    fn run_writes_to_a_file() {
        let dir = std::env::temp_dir().join(format!("caerostris-cli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("graph.jsonl");
        let args = GenerateArgs {
            config: GenConfig::small(20, 40, 3),
            out: Some(path.clone()),
        };
        let mut status: Vec<u8> = Vec::new();
        let stats = run(&args, &mut status).expect("run");
        assert_eq!(stats.nodes_written, 20);
        assert_eq!(stats.edges_written, 40);
        // The file exists and starts with the meta line.
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(
            contents
                .lines()
                .next()
                .unwrap()
                .contains("\"record\":\"meta\"")
        );
        // Status line mentions the destination.
        let status_str = String::from_utf8(status).unwrap();
        assert!(status_str.contains("graph.jsonl"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
