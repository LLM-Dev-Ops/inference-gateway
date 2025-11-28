//! Shell completions command.

use anyhow::Result;
use clap::{Args, CommandFactory};
use clap_complete::{generate, Shell};
use std::io;

use crate::cli::Cli;

/// Arguments for the completions command.
#[derive(Args, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
}

/// Execute the completions command.
pub fn execute(args: CompletionsArgs) -> Result<()> {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();

    generate(args.shell, &mut cmd, name, &mut io::stdout());

    Ok(())
}
