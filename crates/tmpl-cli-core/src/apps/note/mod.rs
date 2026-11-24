//! The `note` app: add, list and show notes.
//!
//! [`exec`] starts the runtime, runs the command inside the audit guard when
//! it mutates, and hands the result to the caller's `finish` before the guard
//! writes its line. Each command is one flow file (`add.rs`, `list.rs`,
//! `show.rs`); the rules they share are in `rules.rs`.

mod add;
mod list;
mod rules;
mod show;

pub use rules::{ListFilter, NewNote, NoteList};

use crate::composition::Env;
use crate::runtime::{self, AuditGuard, AuditMeta, Ctx, Runtime};
use std::path::PathBuf;
use tmpl_cli_store::BadRecord;
use tmpl_cli_store::contracts::{Note, NoteId};

/// What the `note` app can be asked to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Add a note, assigning the next id.
    Add(NewNote),
    /// List the notes matching a filter.
    List(ListFilter),
    /// Show one note.
    Show(NoteId),
}

impl Command {
    /// What the audit log records for this command, or `None` when it is not
    /// audited. Only commands that change durable state are: a read-only
    /// command must not write, not even an audit line (STD-01 R31).
    /// Exhaustive, so a new command cannot compile without deciding
    /// (STD-02 R27).
    pub(crate) fn audit(&self) -> Option<AuditMeta> {
        match self {
            Self::Add(_) => Some(AuditMeta {
                command: "note add",
            }),
            Self::List(_) | Self::Show(_) => None,
        }
    }
}

/// What a `note` command produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The note that was added, and the store file it was written to.
    Added {
        /// The stored note, with its assigned id.
        note: Note,
        /// The store file, so a surface can name where the note went
        /// (STD-01 R30).
        path: PathBuf,
    },
    /// The notes a listing returned.
    Listed {
        /// The selection: the notes returned and how many matched.
        list: NoteList,
        /// The filter it was selected with.
        filter: ListFilter,
        /// Note records isolated as invalid while reading (STD-02 R32).
        bad_records: Vec<BadRecord>,
    },
    /// One note.
    Shown {
        /// The note.
        note: Note,
        /// Other note records isolated as invalid while reading (STD-02 R32).
        bad_records: Vec<BadRecord>,
    },
}

impl Outcome {
    /// The id of the record the command acted on, if it names one, for the
    /// audit log. Exhaustive, so a new outcome decides (STD-02 R27).
    fn target(&self) -> Option<String> {
        match self {
            Self::Added { note, .. } | Self::Shown { note, .. } => Some(note.id.to_string()),
            Self::Listed { .. } => None,
        }
    }
}

/// Why a `note` command failed.
///
/// `#[non_exhaustive]`: this is the library's public boundary (STD-02 R11).
/// Surfaces map it through [`Error::code`] instead of matching variants.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// No note has this id. The message names the store file searched, so a
    /// mis-resolved data directory is visible (STD-02 R26).
    #[error("no note with id {id} in {}", path.display())]
    NotFound {
        /// The id that was looked up.
        id: NoteId,
        /// The store file that was searched.
        path: PathBuf,
    },
    /// The store already holds the largest possible note id (`u64::MAX`).
    #[error(
        "{} cannot allocate another note id: the maximum id ({}) is already in use",
        path.display(),
        u64::MAX
    )]
    IdExhausted {
        /// The store file.
        path: PathBuf,
    },
    /// The runtime could not start, or the store refused or failed.
    #[error(transparent)]
    Runtime(#[from] runtime::Error),
}

impl From<tmpl_cli_store::Error> for Error {
    fn from(err: tmpl_cli_store::Error) -> Self {
        Self::Runtime(runtime::Error::Store(err))
    }
}

impl Error {
    /// A stable `snake_case` code for this failure, for machine output
    /// (STD-01 R19). Adding a code is fine; changing one is a breaking change.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "note_not_found",
            Self::IdExhausted { .. } => "note_id_exhausted",
            Self::Runtime(e) => e.code(),
        }
    }
}

/// Run one `note` command against the environment `env` and pass its result
/// to `finish`.
///
/// A mutating command runs inside the audit guard, created before the flow so
/// an early return or panic is still logged as a failure, and dropped after
/// `finish` so a fail-open audit warning reaches stderr after the command's
/// own payload or error object. A runtime that cannot start is reported to
/// `finish` and audits nothing, since there is no store to audit to.
pub fn exec<R>(env: &Env, command: Command, finish: impl FnOnce(Result<Outcome, Error>) -> R) -> R {
    match Runtime::start(env) {
        Ok(runtime) => run_with(&runtime.ctx(), command, finish),
        Err(err) => finish(Err(err.into())),
    }
}

/// [`exec`] against an already started runtime.
pub(crate) fn run_with<R>(
    ctx: &Ctx<'_>,
    command: Command,
    finish: impl FnOnce(Result<Outcome, Error>) -> R,
) -> R {
    let mut audit = command.audit().map(|meta| {
        AuditGuard::start(ctx.store(), meta, ctx.now(), ctx.actor().map(str::to_owned))
    });
    let result = run(ctx, command);
    if let Some(audit) = audit.as_mut() {
        audit.mark_result(result.as_ref().map(Outcome::target).map_err(Error::code));
    }
    finish(result)
}

/// Run one command without the runtime start or the audit guard.
pub(crate) fn run(ctx: &Ctx<'_>, command: Command) -> Result<Outcome, Error> {
    match command {
        Command::Add(new) => add::add(ctx, new),
        Command::List(filter) => list::list(ctx, filter),
        Command::Show(id) => show::show(ctx, id),
    }
}

#[cfg(test)]
mod tests;
