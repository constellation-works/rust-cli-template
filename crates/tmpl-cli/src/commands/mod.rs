//! Command dispatch: the noun enum and one module per noun.
//!
//! Each noun maps to exactly one app in `tmpl-cli-core`, and lives in its own
//! module with its subcommands, flags, help and handlers (`note.rs`); a noun
//! that outgrows one file becomes a directory (see `note.rs`). Cross-cutting
//! behaviour (the audit log) attaches once, in the core's runtime
//! (STD-02 R24), not here.

mod note;

use crate::output::Output;
use clap::Subcommand;
use tmpl_cli_core::{Env, Error};

pub(crate) use note::NoteCommand;

/// Top-level nouns.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Add, list and show notes
    #[command(subcommand, after_help = note::NOTE_EXAMPLES)]
    Note(NoteCommand),
}

/// Run one command against `env` and pass its outcome to `finish`, which
/// runs before the app's audit guard writes its line.
pub(crate) fn run_with<R>(
    command: Command,
    env: &Env,
    finish: impl FnOnce(Result<Output, Error>) -> R,
) -> R {
    match command {
        Command::Note(command) => note::run(command, env, finish),
    }
}

#[cfg(test)]
mod tests;
