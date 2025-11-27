//! mdbook-validator CLI entry point
//!
//! Implements the mdBook preprocessor protocol:
//! - `mdbook-validator supports <renderer>` - check renderer support
//! - `mdbook-validator` - read JSON from stdin, process, write to stdout

use std::io::{self, Read, Write};
use std::process;

use mdbook::preprocess::{CmdPreprocessor, Preprocessor};
use mdbook_validator::ValidatorPreprocessor;

fn main() {
    tracing_subscriber::fmt().with_writer(io::stderr).init();

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

    let (ctx, book) = CmdPreprocessor::parse_input(io::Cursor::new(&input))?;
    let processed = preprocessor.run(&ctx, book)?;

    let output = serde_json::to_string(&processed)?;
    io::stdout().write_all(output.as_bytes())?;

    Ok(())
}
