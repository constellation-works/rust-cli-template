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
//! lose a note (STD-03 R5, R6). Audit appends take their own `<root>/.audit.lock`
//! so a failed write that timed out on `.lock` still records its failure line
//! within one deadline. New directories are created `0700` and new files `0600`
//! (STD-05 R8), and every load refuses state that is a symbolic link, owned by
//! another user, or writable by group or others (STD-05 R9).

mod audit;
mod format;
mod fsio;

pub use audit::{AuditEvent, AuditStatus};

use crate::error::{BadRecord, Error, RecordLocator, Result};
use crate::note::{NewNote, Note, NoteId};
use crate::query::{self, ListFilter, NoteList};
use audit::AUDIT_FILE;
use format::StoreDocument;
use fsio::LockGuard;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use time::OffsetDateTime;

/// The store file's name under the data directory.
pub const STORE_FILE: &str = "notes.json";

/// The store write lock file's name under the data directory.
const LOCK_FILE: &str = ".lock";

/// The audit-log lock file's name under the data directory, separate from
/// [`LOCK_FILE`] so a lock timeout on a note write does not block its audit line.
const AUDIT_LOCK_FILE: &str = ".audit.lock";

/// How long a writer waits for another writer before refusing (STD-03 R7).
const DEFAULT_LOCK_WAIT: Duration = Duration::from_secs(10);

/// The result of a read operation together with any bad records isolated from
/// the store file (STD-02 R32).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loaded<T> {
    /// The valid value produced by the read.
    pub value: T,
    /// Invalid note records encountered while reading the store; empty when
    /// every record was valid.
    pub bad_records: Vec<BadRecord>,
}

/// A note store rooted at one data directory.
#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
    lock_wait: Duration,
    contended_lock_waits: Arc<AtomicUsize>,
}

impl Store {
    /// A store at `root`. Performs no I/O; the directory is created by the
    /// first write. The caller resolves `root` (STD-02 R3): this crate never
    /// reads the environment or the home directory.
    pub fn open(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            lock_wait: DEFAULT_LOCK_WAIT,
            contended_lock_waits: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Override how long a writer waits for the lock.
    #[must_use]
    pub fn with_lock_wait(mut self, wait: Duration) -> Self {
        self.lock_wait = wait;
        self
    }

    /// Count of contended lock acquisitions observed through this store handle
    /// (and clones of it), used by tests instead of timing lock deadlines.
    #[must_use]
    pub fn contended_lock_waits(&self) -> usize {
        self.contended_lock_waits.load(Ordering::Relaxed)
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

    /// Notes matching `filter`, oldest first, how many matched, and any bad
    /// records isolated from the store file. A missing store is an empty one.
    pub fn list(&self, filter: &ListFilter) -> Result<Loaded<NoteList>> {
        let doc = self.load()?;
        Ok(Loaded {
            value: query::select(doc.notes, filter),
            bad_records: doc.bad_records,
        })
    }

    /// The note with this id, plus any bad records isolated from other entries
    /// in the store file. If `id` belongs to a bad record, returns that
    /// record's typed error.
    pub fn show(&self, id: NoteId) -> Result<Loaded<Note>> {
        let doc = self.load()?;
        if let Some(bad) = doc
            .bad_records
            .iter()
            .find(|r| r.locator == RecordLocator::Id(id))
        {
            return Err(Error::BadRecord(bad.clone()));
        }
        let note = doc
            .notes
            .into_iter()
            .find(|n| n.id == id)
            .ok_or_else(|| Error::NotFound {
                id,
                path: self.path(),
            })?;
        Ok(Loaded {
            value: note,
            bad_records: doc.bad_records,
        })
    }

    /// Add a note, assigning the next id. `now` is the creation time; the
    /// caller supplies it so tests can pin it.
    pub fn add(&self, new: NewNote, now: OffsetDateTime) -> Result<Note> {
        // Every check that can refuse runs before the first write (STD-02
        // R34): an unsafe, corrupt, newer, bad-record or id-exhausted store is
        // refused here, before the directory or the lock file is created.
        let pre = self.load()?;
        if let Some(bad) = pre.bad_records.into_iter().next() {
            return Err(Error::BadRecord(bad));
        }
        if NoteId::next_after(&pre.notes).is_none() {
            return Err(Error::IdExhausted { path: self.path() });
        }
        fsio::create_private_dir(&self.root)?;
        let _lock = self.lock("note add")?;
        // Re-read under the lock: anything read before it may be stale.
        let mut doc = self.load()?;
        if let Some(bad) = doc.bad_records.first() {
            return Err(Error::BadRecord(bad.clone()));
        }
        let id = NoteId::next_after(&doc.notes)
            .ok_or_else(|| Error::IdExhausted { path: self.path() })?;
        let note = new.into_note(id, now);
        doc.notes.push(note.clone());
        let path = self.path();
        fsio::write_atomic(&path, &format::encode(&doc)?)?;
        tracing::debug!(id = %note.id, path = %path.display(), "added note");
        Ok(note)
    }

    /// Append one event to the audit log, creating the data directory and the
    /// log owner-only if needed. The append holds its own `.audit.lock` so
    /// concurrent audit lines never interleave and a store-lock timeout can
    /// still record its failure line immediately. An unsafe data directory is
    /// refused as for any write.
    pub fn record_audit(&self, event: &AuditEvent) -> Result<()> {
        let path = self.audit_path();
        let line = audit::encode_line(event)?;
        if !fsio::inspect_dir(&self.root)? {
            fsio::create_private_dir(&self.root)?;
        }
        let _lock = self.audit_lock("audit")?;
        fsio::append_private(&path, &line)
    }

    fn lock(&self, label: &str) -> Result<LockGuard> {
        LockGuard::acquire_observing(&self.root.join(LOCK_FILE), label, self.lock_wait, || {
            self.contended_lock_waits.fetch_add(1, Ordering::Relaxed);
        })
    }

    fn audit_lock(&self, label: &str) -> Result<LockGuard> {
        LockGuard::acquire_observing(
            &self.root.join(AUDIT_LOCK_FILE),
            label,
            self.lock_wait,
            || {
                self.contended_lock_waits.fetch_add(1, Ordering::Relaxed);
            },
        )
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
