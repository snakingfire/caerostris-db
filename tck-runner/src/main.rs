//! CLI entry point for the openCypher TCK runner.
//!
//! ```text
//! cargo run -p tck-runner                         # run vendored corpus, print summary
//! cargo run -p tck-runner -- <features-dir>       # run a specific corpus/dir/file
//! cargo run -p tck-runner -- --format json        # emit JSON to stdout
//! cargo run -p tck-runner -- --output report.json # also write JSON to a file
//! ```
//!
//! Exit status: `0` when the suite *ran and a report was produced* — `pending`
//! scenarios and the known parser-coverage gap (BUG-0018, `Literals6.feature`)
//! are expected during the phased ramp and do not fail the run. The runner's job
//! is to *report* the number; conformance regressions (count drift, new parse
//! errors, any `fail`) are caught by the test suite (`tests/vendored_corpus.rs`)
//! that CI also runs. Pass `--strict` to instead exit non-zero on any `fail` or
//! `parse_errors` (intended for once BUG-0018 is closed and the corpus parses
//! fully).
//! A non-zero exit otherwise means an operational error (corpus unreadable,
//! output unwritable).

use std::path::PathBuf;
use std::process::ExitCode;

use tck_runner::engine::PendingEngine;
use tck_runner::report::Report;
use tck_runner::runner::run_suite;
// `read_provenance` is referenced via its full path at the call site.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Text,
    Json,
}

#[derive(Debug)]
struct Args {
    features_dir: PathBuf,
    format: Format,
    output: Option<PathBuf>,
    strict: bool,
}

fn parse_args() -> Result<Args, String> {
    parse_args_from(std::env::args().skip(1))
}

/// Argument parsing over an arbitrary token stream, so it is unit-testable
/// without touching the process environment.
fn parse_args_from<I: IntoIterator<Item = String>>(args: I) -> Result<Args, String> {
    let mut features_dir: Option<PathBuf> = None;
    let mut format = Format::Text;
    let mut output: Option<PathBuf> = None;
    let mut strict = false;

    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--strict" => strict = true,
            "--format" => {
                let v = it.next().ok_or("--format requires a value (text|json)")?;
                format = match v.as_str() {
                    "text" => Format::Text,
                    "json" => Format::Json,
                    other => return Err(format!("unknown --format '{other}' (text|json)")),
                };
            }
            "--output" | "-o" => {
                output = Some(PathBuf::from(
                    it.next().ok_or("--output requires a file path")?,
                ));
            }
            "-h" | "--help" => return Err("help".to_string()),
            other if other.starts_with('-') => {
                return Err(format!("unknown flag '{other}'"));
            }
            other => {
                if features_dir.is_some() {
                    return Err(format!("unexpected extra argument '{other}'"));
                }
                features_dir = Some(PathBuf::from(other));
            }
        }
    }

    Ok(Args {
        features_dir: features_dir.unwrap_or_else(tck_runner::default_features_dir),
        format,
        output,
        strict,
    })
}

fn usage() {
    eprintln!(
        "Usage: tck-runner [FEATURES_DIR] [--format text|json] [--output FILE]\n\
         \n\
         Runs the openCypher TCK against the caerostris-db engine adapter and\n\
         reports pass / pending / fail counts and the pass-rate.\n\
         \n\
         FEATURES_DIR  directory of .feature files (default: vendored TCK corpus)\n\
         --format      output format: text (default) or json\n\
         --output      also write the JSON report to FILE\n\
         --strict      exit non-zero on any fail or parse_errors (default: off)"
    );
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            if msg != "help" {
                eprintln!("error: {msg}\n");
            }
            usage();
            return if msg == "help" {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            };
        }
    };

    // The engine adapter is the stub until EPIC-002 plugs in a real engine.
    let summary = match run_suite(&args.features_dir, || PendingEngine) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "error: failed to read corpus at {}: {e}",
                args.features_dir.display()
            );
            return ExitCode::FAILURE;
        }
    };

    let provenance = tck_runner::read_provenance(&args.features_dir);
    let report = Report::with_provenance(&summary, provenance);
    let json = report.to_json();

    if let Some(path) = &args.output
        && let Err(e) = std::fs::write(path, format!("{json}\n"))
    {
        eprintln!("error: failed to write {}: {e}", path.display());
        return ExitCode::FAILURE;
    }

    match args.format {
        Format::Json => println!("{json}"),
        Format::Text => {
            println!(
                "openCypher TCK results (corpus: {})",
                args.features_dir.display()
            );
            if let Some(tag) = &report.provenance.tck_tag {
                println!("  pinned tag:   {tag}");
            }
            println!("  total:        {}", report.total);
            println!("  pass:         {}", report.pass);
            println!("  pending:      {}", report.pending);
            println!("  fail:         {}", report.fail);
            println!("  parse_errors: {}", report.parse_errors);
            println!(
                "  pass_rate:    {:.4} ({:.2}%)",
                report.pass_rate,
                report.pass_rate * 100.0
            );
        }
    }

    // The run succeeded: a report was produced. `pending` and the known parser
    // gap (BUG-0018) are expected during the phased ramp and do not fail the
    // run — conformance regressions are caught by the test suite. Only
    // `--strict` turns fails/parse_errors into a non-zero exit.
    if args.strict && (report.fail > 0 || report.parse_errors > 0) {
        eprintln!(
            "strict mode: {} fail, {} parse_errors",
            report.fail, report.parse_errors
        );
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(tokens: &[&str]) -> Result<Args, String> {
        parse_args_from(tokens.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults_to_vendored_corpus_text_no_output() {
        let a = args(&[]).unwrap();
        assert_eq!(a.format, Format::Text);
        assert!(a.output.is_none());
        assert!(!a.strict);
        assert!(a.features_dir.ends_with("tck/openCypher/features"));
    }

    #[test]
    fn positional_features_dir_is_honored() {
        let a = args(&["/some/dir"]).unwrap();
        assert_eq!(a.features_dir, PathBuf::from("/some/dir"));
    }

    #[test]
    fn json_format_and_output_and_strict_parse() {
        let a = args(&["--format", "json", "--output", "/tmp/r.json", "--strict"]).unwrap();
        assert_eq!(a.format, Format::Json);
        assert_eq!(a.output, Some(PathBuf::from("/tmp/r.json")));
        assert!(a.strict);
    }

    #[test]
    fn unknown_flag_is_rejected() {
        assert!(args(&["--nope"]).is_err());
    }

    #[test]
    fn unknown_format_value_is_rejected() {
        assert!(args(&["--format", "yaml"]).is_err());
    }

    #[test]
    fn missing_format_value_is_rejected() {
        assert!(args(&["--format"]).is_err());
    }

    #[test]
    fn two_positionals_are_rejected() {
        assert!(args(&["a", "b"]).is_err());
    }

    #[test]
    fn help_is_signalled() {
        assert_eq!(args(&["--help"]).unwrap_err(), "help");
    }
}
