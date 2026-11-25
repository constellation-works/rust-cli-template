//! One repository per record.
//!
//! A repository is named for the record it holds, never for a command: it
//! gets, lists and puts records and decides nothing about them. It is one
//! record's use of a driver's handle (`crate::driver`): a record's
//! repository holds its SQL and row mapping and runs it on the connection
//! the store passes it. A caller reaches a record only through
//! `snapshot.<record>()` in a read and `tx.<record>()` in a write, so it
//! composes repositories in one read or one write, and no connection, path
//! or borrowed document crosses into its hands.

// example:begin
pub(crate) mod note;

pub use note::{Notes, NotesMut};
// example:end

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

    /// The database this read opened.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The database's schema version, the store format, or `None` when the
    /// store holds no database yet.
    #[must_use]
    pub fn format(&self) -> Option<u32> {
        self.reader.as_ref().map(Reader::version)
    }
}

// example:begin
impl Snapshot {
    /// The notes repository over this snapshot.
    #[must_use]
    pub fn notes(&self) -> Notes<'_> {
        Notes::new(self.reader.as_deref(), &self.path)
    }
}
// example:end

/// The store inside one write transaction.
#[derive(Debug)]
#[allow(
    dead_code,
    reason = "the record repositories read both fields; a store with none yet reads neither"
)]
pub struct Tx<'a> {
    conn: &'a Connection,
    path: &'a Path,
}

impl<'a> Tx<'a> {
    pub(crate) fn new(conn: &'a Connection, path: &'a Path) -> Self {
        Self { conn, path }
    }
}

// example:begin
impl Tx<'_> {
    /// The notes repository inside this write.
    pub fn notes(&mut self) -> NotesMut<'_> {
        NotesMut::new(self.conn, self.path)
    }
}
// example:end
