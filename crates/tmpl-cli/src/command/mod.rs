//! Maps command arguments to apps and payloads; imports error and output.
//!
//! Command dispatch: the noun enum and one module per noun.
//!
//! Each noun maps to exactly one app in `tmpl-cli-core`, and lives in its own
//! module with its subcommands, flags, help and handlers; a noun that
//! outgrows one file becomes a directory with one file per verb. Cross-cutting
//! behaviour (the audit log) attaches once, in the kit's chokepoint that
//! every app's `exec` forwards to (STD-02 R24), not here.

mod common;

mod status;

// example:begin
mod note;
// example:end

use crate::output::Output;
use clap::Subcommand;
use tmpl_cli_core::{AuditWarning, Error, Origin, Runtime};

pub(crate) use status::StatusArgs;
// example:begin
pub(crate) use note::{AddArgs, ListArgs, NoteCommand, ShowArgs};
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

/// Run one command against `runtime` for the client `origin`, render its
/// outcome with `finish`, then settle its audit record: the line follows the
/// output, and a line that could not be written comes back as the warning.
pub(crate) fn run<R>(
    command: Command,
    runtime: &Runtime,
    origin: &Origin,
    finish: impl FnOnce(Result<Output, Error>) -> R,
) -> (R, Option<AuditWarning>) {
    match command {
        Command::Status(args) => status::run(args, runtime, origin, finish),
        // example:begin
        Command::Note(command) => note::run(command, runtime, origin, finish),
        // example:end
    }
}
