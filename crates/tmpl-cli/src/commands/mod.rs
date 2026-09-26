//! Command dispatch: the noun enum, the one chokepoint every command runs
//! through, and one module per noun.
//!
//! [`run`] is where cross-cutting behaviour attaches, once, for every command
//! (STD-02 R24): today the audit log, through [`crate::audit_middleware`].
//! Each noun lives in its own module with its subcommands, flags, help and
//! handlers (`note.rs`); a noun that outgrows one file becomes a directory
//! (see `note.rs`).

mod note;

use crate::app::Context;
use crate::audit_middleware::{AuditGuard, AuditMeta};
use crate::error::CliError;
use crate::output::Output;
use clap::Subcommand;

pub(crate) use note::NoteCommand;

/// Top-level nouns.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Add, list and show notes
    #[command(subcommand, after_help = note::NOTE_EXAMPLES)]
    Note(NoteCommand),
}

impl Command {
    /// What the audit log records for this command, or `None` when it is not
    /// audited. Only commands that change durable state are: a read-only
    /// command must not write, not even an audit line (STD-01 R31).
    /// Exhaustive, so a new command cannot compile without deciding
    /// (STD-02 R27).
    pub(crate) fn audit(&self) -> Option<AuditMeta> {
        match self {
            Self::Note(command) => command.audit(),
        }
    }
}

/// Run one command, auditing it if it is audited.
///
/// The guard exists before dispatch and records when it is dropped, so a
/// command that returns early or panics is still logged, as a failure.
pub(crate) fn run(command: Command, ctx: &Context) -> Result<Output, CliError> {
    let mut audit = command
        .audit()
        .map(|meta| AuditGuard::start(ctx.store(), meta, ctx.now()));
    let result = match command {
        Command::Note(command) => note::run(command, ctx),
    };
    if let Some(audit) = audit.as_mut() {
        audit.mark_result(&result);
    }
    result
}

#[cfg(test)]
mod tests;
