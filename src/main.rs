//! mdbook-validator CLI entry point
//!
//! Implements the mdBook preprocessor protocol:
//! - `mdbook-validator supports <renderer>` - check renderer support
//! - `mdbook-validator` - read JSON from stdin, process, write to stdout

use std::io::{self, Read, Write};
use std::process;

use mdbook::preprocess::{CmdPreprocessor, Preprocessor};
use mdbook_validator::dependency::{check_all, RealChecker};
use mdbook_validator::ValidatorPreprocessor;

fn main() {
    tracing_subscriber::fmt().with_writer(io::stderr).init();

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
            if preprocessor.supports_renderer(&renderer) {
                process::exit(0);
            } else {
                process::exit(1);
            }
        }
    }

    // No subcommand - run as preprocessor
    if let Err(e) = run_preprocessor(&preprocessor) {
        tracing::error!("Preprocessor error: {e}");
        process::exit(1);
    }
}

fn run_preprocessor(preprocessor: &ValidatorPreprocessor) -> Result<(), mdbook::errors::Error> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    // Debug: log input length and first 200 chars
    tracing::warn!(
        "DEBUG stdin: {} bytes. First 200 chars: {:?}",
        input.len(),
        &input[..input.len().min(200)]
    );

    let (ctx, book) = CmdPreprocessor::parse_input(io::Cursor::new(&input))?;
    let processed = preprocessor.run(&ctx, book)?;

    let output = serde_json::to_string(&processed)?;
    io::stdout().write_all(output.as_bytes())?;

    Ok(())
}
