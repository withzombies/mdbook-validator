#![doc = include_str!("../README.md")]

//! mdbook-validator CLI entry point
//!
//! Implements the mdBook preprocessor protocol:
//! - `mdbook-validator supports <renderer>` - check renderer support
//! - `mdbook-validator` - read JSON from stdin, process, write to stdout

use std::io::{self, Read, Write};
use std::process;

use mdbook_preprocessor::{parse_input, Preprocessor};
use mdbook_validator::dependency::{check_all, RealChecker};
use mdbook_validator::ValidatorPreprocessor;
use tracing_subscriber::EnvFilter;

/// Initialize the logging subsystem.
///
/// Uses `MDBOOK_LOG` environment variable to control log levels (same as mdbook).
/// Defaults to INFO level if not set. Invalid values are handled gracefully.
///
/// # Panics
///
/// Panics if called more than once (tracing subscriber already initialized).
fn init_logger() {
    let filter = EnvFilter::builder()
        .with_env_var("MDBOOK_LOG")
        .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
        .from_env_lossy();

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .with_target(true)
        .without_time()
        .init();
}

fn main() {
    init_logger();

    // Check for required external dependencies and warn if missing
    let status = check_all(&RealChecker);
    if !status.jq_available {
        tracing::warn!(
            "jq is not installed. JSON validators (sqlite, osquery, osquery-config, bash-exec) \
             will fail. Install with: brew install jq (macOS) or apt-get install jq (Linux)"
        );
    }
    if !status.docker_available {
        tracing::warn!(
            "Docker is not running. Container-based validators will fail. \
             Please start Docker Desktop or the Docker daemon."
        );
    }

    let preprocessor = ValidatorPreprocessor::new();

    if let Some(sub_cmd) = std::env::args().nth(1) {
        if sub_cmd == "supports" {
            let renderer = std::env::args().nth(2).unwrap_or_default();
            match preprocessor.supports_renderer(&renderer) {
                Ok(true) => process::exit(0),
                Ok(false) | Err(_) => process::exit(1),
            }
        }
    }

    // No subcommand - run as preprocessor
    if let Err(e) = run_preprocessor(&preprocessor) {
        tracing::error!("Preprocessor error: {e}");
        process::exit(1);
    }
}

fn run_preprocessor(
    preprocessor: &ValidatorPreprocessor,
) -> Result<(), mdbook_preprocessor::errors::Error> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    let (ctx, book) = parse_input(io::Cursor::new(&input))?;
    let processed = preprocessor.run(&ctx, book)?;

    let output = serde_json::to_string(&processed)?;
    io::stdout().write_all(output.as_bytes())?;

    Ok(())
}
