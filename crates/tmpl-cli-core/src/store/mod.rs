//! Persistence: the file-backed note store and everything that touches disk.
//!
//! The rest of the crate is business logic with no I/O ([`crate::note`],
//! [`crate::query`]); this module is the only one that reads or writes files,
//! so a second backend, or a move to a database, replaces this module and
//! nothing else. It is split by responsibility:
//!
//! - this file, the [`Store`] facade: load, apply the domain rules, persist;
//! - `format`: the persisted document, its format versions and upgrades;
//! - `audit`: the audit log's record shape;
//! - `fsio`: durable writes, appends, locking and private-state checks,
//!   mechanism only, with no knowledge of notes.
//!
//! One JSON document, `<root>/notes.json`, holds every note. Reads take no
//! lock and write nothing (STD-01 R31): the file is only ever replaced by an
//! atomic rename, so a reader sees a whole old version or a whole new one.
//! Writes are a read-modify-write, so they hold `<root>/.lock` from the read
//! to the rename; two concurrent `add`s therefore never assign the same id or
//! lose a note (STD-03 R5, R6). The directory is `0700` and the files `0600`,
//! and a load refuses state that is not (STD-05 R8, R9).

mod audit;
mod format;
mod fsio;

pub use audit::{AUDIT_FILE, AuditEvent, AuditStatus};

use crate::error::{Error, Result};
use crate::note::{NewNote, Note, NoteId};
use crate::query::{self, ListFilter, NoteList};
use format::StoreDocument;
use fsio::LockGuard;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;
use time::OffsetDateTime;

/// The store file's name under the data directory.
pub const STORE_FILE: &str = "notes.json";

/// The lock file's name under the data directory.
const LOCK_FILE: &str = ".lock";

/// How long a writer waits for another writer before refusing (STD-03 R7).
pub const DEFAULT_LOCK_WAIT: Duration = Duration::from_secs(10);

/// A note store rooted at one data directory.
#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
    lock_wait: Duration,
}

impl Store {
    /// A store at `root`. Performs no I/O; the directory is created by the
    /// first write. The caller resolves `root` (STD-02 R3): this crate never
    /// reads the environment or the home directory.
    pub fn open(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            lock_wait: DEFAULT_LOCK_WAIT,
        }
    }

    /// Override how long a writer waits for the lock.
    #[must_use]
    pub fn with_lock_wait(mut self, wait: Duration) -> Self {
        self.lock_wait = wait;
        self
    }

    /// The data directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The store file, `<root>/notes.json`.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.root.join(STORE_FILE)
    }

    /// The audit log, `<root>/audit.jsonl`.
    #[must_use]
    pub fn audit_path(&self) -> PathBuf {
        self.root.join(AUDIT_FILE)
    }

    /// Notes matching `filter`, oldest first, and how many matched. A missing
    /// store is an empty one.
    pub fn list(&self, filter: &ListFilter) -> Result<NoteList> {
        Ok(query::select(self.load()?.notes, filter))
    }

    /// The note with this id.
    pub fn show(&self, id: NoteId) -> Result<Note> {
        self.load()?
            .notes
            .into_iter()
            .find(|n| n.id == id)
            .ok_or_else(|| Error::NotFound {
                id,
                path: self.path(),
            })
    }

    /// Add a note, assigning the next id. `now` is the creation time; the
    /// caller supplies it so tests can pin it.
    pub fn add(&self, new: NewNote, now: OffsetDateTime) -> Result<Note> {
        // Every check that can refuse runs before the first write (STD-02
        // R34): an unsafe, corrupt or newer store is refused here, before the
        // directory or the lock file is created.
        self.load()?;
        fsio::create_private_dir(&self.root)?;
        let _lock = self.lock("note add")?;
        // Re-read under the lock: anything read before it may be stale.
        let mut doc = self.load()?;
        let note = new.into_note(NoteId::next_after(&doc.notes), now);
        doc.notes.push(note.clone());
        let path = self.path();
        fsio::write_atomic(&path, &format::encode(&path, doc)?)?;
        tracing::debug!(id = %note.id, path = %path.display(), "added note");
        Ok(note)
    }

    /// Append one event to the audit log, creating the data directory and the
    /// log owner-only if needed. The append holds the store lock, so events
    /// from concurrent commands never interleave. An unsafe data directory is
    /// refused as for any write.
    pub fn record_audit(&self, event: &AuditEvent) -> Result<()> {
        let path = self.audit_path();
        let line = audit::encode_line(event)
            .map_err(|e| Error::io("encode", path.clone(), io::Error::other(e)))?;
        if !fsio::inspect_dir(&self.root)? {
            fsio::create_private_dir(&self.root)?;
        }
        let _lock = self.lock("audit")?;
        fsio::append_private(&path, &line)
    }

    fn lock(&self, label: &str) -> Result<LockGuard> {
        LockGuard::acquire(&self.root.join(LOCK_FILE), label, self.lock_wait)
    }

    fn load(&self) -> Result<StoreDocument> {
        let path = self.path();
        if !fsio::inspect_dir(&self.root)? || !fsio::inspect_file(&path)? {
            return Ok(StoreDocument::empty());
        }
        let bytes = fs::read(&path).map_err(|e| Error::io("read", &path, e))?;
        format::decode(&path, &bytes)
    }
}

#[cfg(test)]
mod tests;
