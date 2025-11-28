//! LLM Inference Gateway CLI
//!
//! Command-line interface for managing and interacting with the LLM Inference Gateway.

use anyhow::Result;
use clap::Parser;

mod cli;
mod commands;
mod output;

use cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if present
    let _ = dotenvy::dotenv();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize tracing
    init_tracing(cli.verbose, cli.json);

    // Execute command
    cli.execute().await
}

/// Initialize tracing/logging based on verbosity and format.
fn init_tracing(verbose: u8, json: bool) {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = match verbose {
        0 => EnvFilter::new("warn"),
        1 => EnvFilter::new("info"),
        2 => EnvFilter::new("debug"),
        _ => EnvFilter::new("trace"),
    };

    let subscriber = tracing_subscriber::registry().with(filter);

    if json {
        subscriber
            .with(fmt::layer().json())
            .init();
    } else {
        subscriber
            .with(fmt::layer().with_target(verbose > 1))
            .init();
    }
}
