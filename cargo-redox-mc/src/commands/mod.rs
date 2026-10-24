//! Subcommand implementations.

pub mod build;
pub mod new;
pub mod run;

use crate::cli::{Cli, Command};
use crate::error::CliError;

/// Dispatches a parsed command line to its implementation.
pub fn dispatch(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::New(args) => new::execute(args),
        Command::Build(args) => build::execute(args),
        Command::Run(args) => run::execute(args),
    }
}
