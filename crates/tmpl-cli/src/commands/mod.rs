//! Command dispatch: the noun enum, the one chokepoint every command runs
//! through, and one module per noun.
//!
//! [`run_with`] is where cross-cutting behaviour attaches, once, for every
//! command (STD-02 R24): today the audit log, through
//! [`crate::audit_middleware`]. Each noun lives in its own module with its
//! subcommands, flags, help and handlers (`note.rs`); a noun that outgrows
//! one file becomes a directory (see `note.rs`).

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

/// Run one command, auditing it if it is audited, and pass its outcome to
/// `finish` before the audit guard drops.
///
/// Creating the guard before dispatch ensures an early return or panic is
/// still logged as a failure; dropping it after `finish` ensures any
/// fail-open audit warning reaches stderr after the command's own payload or
/// JSON error object.
pub(crate) fn run_with<R>(
    command: Command,
    ctx: &Context,
    finish: impl FnOnce(Result<Output, CliError>) -> R,
) -> R {
    let mut audit = command.audit().map(|meta| {
        AuditGuard::start(ctx.store(), meta, ctx.now(), ctx.actor().map(str::to_owned))
    });
    let result = match command {
        Command::Note(command) => note::run(command, ctx),
    };
    if let Some(audit) = audit.as_mut() {
        audit.mark_result(&result);
    }
    finish(result)
}

#[cfg(test)]
mod tests;
