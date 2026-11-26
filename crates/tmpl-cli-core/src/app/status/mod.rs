//! The `status` app: where the data directory is and which store format it
//! holds. Read-only, so never audited (STD-01 R31).
//!
//! [`exec`] forwards to the kit's chokepoint, [`cw_app::exec`], like every
//! app's; the app supplies what differs by implementing [`cw_app::App`] on
//! [`StatusApp`]. The one flow is `show.rs`.

mod show;

use crate::composition::{Ctx, Runtime};
use cw_app::{App, AuditMeta, Finished, Origin};
use cw_base::Kind;
use std::path::PathBuf;
use tmpl_cli_store::Store;

/// What the `status` app can be asked to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Report the data directory and the store format.
    Show,
}

impl Command {
    /// What the audit log records for this command: nothing, since no
    /// `status` command changes durable state (STD-01 R31). Exhaustive, so a
    /// new command cannot compile without deciding (STD-02 R27).
    pub(crate) fn audit(self) -> Option<AuditMeta> {
        match self {
            Self::Show => None,
        }
    }
}

/// What a `status` command produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The store as one read saw it.
    Shown {
        /// The data directory the command resolved.
        root: PathBuf,
        /// The database's store format (its schema version), or `None` when
        /// the data directory holds no database yet.
        format: Option<u32>,
    },
}

/// Why a `status` command failed, as the app's own typed error.
///
/// [`exec`] returns the workspace's error type; this enum crosses into it
/// through the one translator below (STD-02 R12). `#[non_exhaustive]`: it is
/// public (STD-02 R11).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
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
            Self::Store(err) => err.code(),
        }
    }

    /// How this failure is classified. Exhaustive, so a new variant decides
    /// (STD-02 R27).
    #[must_use]
    pub fn kind(&self) -> Kind {
        match self {
            Self::Store(err) => err.kind(),
        }
    }
}

/// The `status` app's one translator into the workspace's error type
/// (STD-02 R12). A store failure crosses as the store's own error, so its
/// typed form stays reachable.
impl From<Error> for cw_base::Error {
    fn from(err: Error) -> Self {
        match err {
            Error::Store(store) => store,
        }
    }
}

/// The `status` app as the kit's chokepoint sees it.
#[derive(Debug)]
pub(crate) struct StatusApp;

impl App for StatusApp {
    type State = Store;
    type Command = Command;
    type Outcome = Outcome;
    type Error = Error;

    fn audit(command: &Command) -> Option<AuditMeta> {
        command.audit()
    }

    /// Dispatch only: the error translation is the chokepoint's.
    fn run(ctx: &Ctx<'_>, command: Command) -> Result<Outcome, Error> {
        match command {
            Command::Show => show::show(ctx),
        }
    }

    fn target(_outcome: &Outcome) -> Option<String> {
        None
    }
}

/// Run one `status` command through the kit's chokepoint, for the client
/// `origin`. No `status` command is audited, so settling the returned
/// [`Finished`] writes nothing.
pub fn exec(runtime: &Runtime, origin: &Origin, command: Command) -> Finished<Outcome> {
    cw_app::exec::<StatusApp>(runtime, origin, command)
}
