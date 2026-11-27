//! The `note` app: add, list and show notes.
//!
//! [`exec`] forwards to the kit's chokepoint, [`cw_app::exec`], which starts
//! the audit record of a mutating command, runs it and returns a
//! [`Finished`]: the surface renders the result, then settles the record.
//! The app supplies what differs by implementing [`cw_app::App`] on
//! [`NoteApp`]. Each command is one flow file
//! (`add.rs`, `list.rs`, `show.rs`); the rules they share are in `rule.rs`.

mod add;

mod list;

mod rule;

mod show;

pub use rule::{DEFAULT_LIMIT, Limit, ListFilter, NewNote, NoteList};

use crate::composition::{Ctx, Runtime};
use cw_app::{App, AuditMeta, Finished, Origin};
use cw_base::Kind;
use std::path::PathBuf;
use tmpl_cli_store::contract::{Note, NoteId};
use tmpl_cli_store::{BadRecord, Store};

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
            Self::Add(_) => Some(AuditMeta::new("note add")),
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
        /// The notes database, so a surface can name where the note went
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

/// Why a `note` command failed, as the app's own typed error.
///
/// [`exec`] returns the workspace's error type; this enum crosses into it
/// through the one translator below (STD-02 R12), and a caller that knows the
/// app reads it back with `downcast_ref`. `#[non_exhaustive]`: it is public
/// (STD-02 R11).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// No note has this id. The message names the store file searched, so a
    /// mis-resolved data directory is visible (STD-02 R26).
    #[error("no note with id {id} in {}", path.display())]
    NotFound {
        /// The id that was looked up.
        id: NoteId,
        /// The notes database that was searched.
        path: PathBuf,
    },
    /// The store already holds the largest possible note id
    /// ([`NoteId::MAX`]).
    #[error(
        "{} cannot allocate another note id: the maximum id ({}) is already in use",
        path.display(),
        NoteId::MAX
    )]
    IdExhausted {
        /// The notes database.
        path: PathBuf,
    },
    /// The store refused or failed: the store's typed error, already in the
    /// workspace's error type, as every store operation returns it.
    #[error(transparent)]
    Store(#[from] cw_base::Error),
}

impl Error {
    /// A stable `snake_case` code for this failure, for machine output
    /// (STD-01 R19). Adding a code is fine; changing one is a breaking change.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "note_not_found",
            Self::IdExhausted { .. } => "note_id_exhausted",
            Self::Store(err) => err.code(),
        }
    }

    /// How this failure is classified. Exhaustive, so a new variant decides
    /// (STD-02 R27).
    #[must_use]
    pub fn kind(&self) -> Kind {
        match self {
            Self::NotFound { .. } => Kind::NotFound,
            Self::IdExhausted { .. } => Kind::Refused,
            Self::Store(err) => err.kind(),
        }
    }
}

/// The `note` app's one translator into the workspace's error type
/// (STD-02 R12). A store failure crosses as the store's own error, so its
/// typed form stays reachable.
impl From<Error> for cw_base::Error {
    fn from(err: Error) -> Self {
        match err {
            Error::Store(store) => store,
            err @ (Error::NotFound { .. } | Error::IdExhausted { .. }) => {
                Self::new(err.kind(), err.code(), err)
            }
        }
    }
}

/// The `note` app as the kit's chokepoint sees it.
#[derive(Debug)]
pub(crate) struct NoteApp;

impl App for NoteApp {
    type State = Store;
    type Command = Command;
    type Outcome = Outcome;
    type Error = Error;

    fn audit(command: &Command) -> Option<AuditMeta> {
        command.audit()
    }

    /// Dispatch only: the audit record and the error translation are the
    /// chokepoint's.
    fn run(ctx: &Ctx<'_>, command: Command) -> Result<Outcome, Error> {
        match command {
            Command::Add(new) => add::add(ctx, new),
            Command::List(filter) => list::list(ctx, filter),
            Command::Show(id) => show::show(ctx, id),
        }
    }

    fn target(outcome: &Outcome) -> Option<String> {
        outcome.target()
    }
}

/// Run one `note` command through the kit's chokepoint, for the client
/// `origin`.
///
/// A mutating command's audit record starts before it runs, as a failure
/// until it reports otherwise; a read-only command writes nothing. The
/// surface renders the [`Finished`] result first, then settles the record,
/// so the audit line follows the output and a failure to write it comes
/// back as a warning value.
pub fn exec(runtime: &Runtime, origin: &Origin, command: Command) -> Finished<Outcome> {
    cw_app::exec::<NoteApp>(runtime, origin, command)
}

#[cfg(test)]
mod tests;
