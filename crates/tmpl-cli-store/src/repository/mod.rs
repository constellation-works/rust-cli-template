//! One repository per record: `notes` and `audit`.
//!
//! A repository is named for the record it holds, never for a command: it
//! gets, lists and puts records and decides nothing about them. It is one
//! record's use of a driver's handle (`crate::driver`): the notes repository
//! holds the notes' SQL and row mapping and runs it on the connection the
//! store passes it. A caller reaches a record only through
//! [`Snapshot::notes`] in a read and [`Tx::notes`] in a write, so it
//! composes repositories in one read or one write, and no connection, path
//! or borrowed document crosses into its hands.

mod audit;
mod notes;

pub use audit::Audit;
pub use notes::{Notes, NotesMut};

pub(crate) use notes::first_bad;

use cw_sqlite::Reader;
use cw_sqlite::rusqlite::Connection;
use std::path::{Path, PathBuf};

/// The store as one read saw it: one read-only connection holding one read
/// transaction, so every repository call on it sees the same version.
#[derive(Debug)]
pub struct Snapshot {
    /// `None` when the store does not exist yet: it reads as empty.
    reader: Option<Reader>,
    path: PathBuf,
}

impl Snapshot {
    pub(crate) fn new(reader: Option<Reader>, path: PathBuf) -> Self {
        Self { reader, path }
    }

    /// The notes repository over this snapshot.
    #[must_use]
    pub fn notes(&self) -> Notes<'_> {
        Notes::new(self.reader.as_deref(), &self.path)
    }
}

/// The store inside one write transaction.
#[derive(Debug)]
pub struct Tx<'a> {
    conn: &'a Connection,
    path: &'a Path,
}

impl<'a> Tx<'a> {
    pub(crate) fn new(conn: &'a Connection, path: &'a Path) -> Self {
        Self { conn, path }
    }

    /// The notes repository inside this write.
    pub fn notes(&mut self) -> NotesMut<'_> {
        NotesMut::new(self.conn, self.path)
    }
}
