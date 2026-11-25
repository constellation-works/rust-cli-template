//! Command dispatch: the noun enum and one module per noun.
//!
//! Each noun maps to exactly one app in `tmpl-cli-core`, and lives in its own
//! module with its subcommands, flags, help and handlers; a noun that
//! outgrows one file becomes a directory with one file per verb. Cross-cutting
//! behaviour (the audit log) attaches once, in the core's runtime
//! (STD-02 R24), not here.

mod status;

use crate::output::Output;
use clap::Subcommand;
use tmpl_cli_core::{Env, Error};

pub(crate) use status::StatusArgs;
// example:begin

mod note;

pub(crate) use note::NoteCommand;
// example:end

/// Top-level nouns.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Show the data directory and the store format
    #[command(after_help = status::STATUS_EXAMPLES)]
    Status(StatusArgs),
    // example:begin
    /// Add, list and show notes
    #[command(subcommand, after_help = note::NOTE_EXAMPLES)]
    Note(NoteCommand),
    // example:end
}

/// Run one command against `env` and pass its outcome to `finish`, which
/// runs before the app's audit guard writes its line.
pub(crate) fn run_with<R>(
    command: Command,
    env: &Env,
    finish: impl FnOnce(Result<Output, Error>) -> R,
) -> R {
    match command {
        Command::Status(args) => status::run(args, env, finish),
        // example:begin
        Command::Note(command) => note::run(command, env, finish),
        // example:end
    }
}

#[cfg(test)]
mod tests;
