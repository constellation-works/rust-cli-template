//! The [`Store`] facade: one data directory, read as a snapshot, written in
//! one locked transaction.
//!
//! One JSON document, `<root>/notes.json`, holds every note. Reads take no
//! lock and write nothing (STD-01 R31): the file is only ever replaced by an
//! atomic rename, so a reader sees a whole old version or a whole new one.
//! A write is a read-modify-write, so [`Store::write`] holds `<root>/.lock`
//! from the read to the rename; two concurrent writers therefore never lose
//! each other's records (STD-03 R5, R6). Audit appends take their own
//! `<root>/.audit.lock` (see the audit repository) so a failed write that
//! timed out on `.lock` still records its failure line within one deadline.
//! New directories are created `0700` and new files `0600` (STD-05 R8), and
//! every load refuses state that is a symbolic link, owned by another user,
//! or open to any group or other permission bit (STD-05 R9). The mechanism
//! is the kit's `cw_fs`; the store names the files and the codes.
//!
//! The store decides nothing about notes: which id comes next, which notes a
//! listing returns and when a command is refused are the note app's rules.

use crate::db::format::{self, StoreDocument};
use crate::db::{AUDIT_LOCK_FILE, LOCK_FILE, STORE_CAP, STORE_FILE, audit};
use crate::error::{Error, Result};
use crate::repository::{Audit, Snapshot, Tx};
use cw_fs::Lock;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long a writer waits for another writer before refusing (STD-03 R7).
const DEFAULT_LOCK_WAIT: Duration = Duration::from_secs(10);

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
        self.root.join(audit::AUDIT_FILE)
    }

    /// Everything the store holds, read once. A missing store reads as an
    /// empty one. Takes no lock and writes nothing.
    pub fn read(&self) -> Result<Snapshot> {
        Ok(Snapshot::new(self.load()?))
    }

    /// Change the store in one transaction: create the data directory if
    /// needed, take `.lock` (recording `label` as the holder), re-read the
    /// store under the lock, run `apply` against it, and replace the file
    /// atomically if `apply` succeeds. Nothing is written when `apply` fails.
    ///
    /// A store holding bad records is refused (`store_bad_record`) before
    /// `apply` runs: the rewrite would drop them, which is data loss
    /// (STD-02 R16, R31). Checks a caller wants made before the directory or
    /// lock file is created belong in its own read before this call
    /// (STD-02 R34).
    pub fn write<T, E>(
        &self,
        label: &str,
        apply: impl FnOnce(&mut Tx<'_>) -> std::result::Result<T, E>,
    ) -> std::result::Result<T, E>
    where
        E: From<Error>,
    {
        cw_fs::create_private_dir(&self.root).map_err(Error::from)?;
        let _lock = self.lock(label)?;
        // Re-read under the lock: anything read before it may be stale.
        let mut doc = self.load()?;
        if let Some(bad) = doc.bad_records.first() {
            return Err(Error::BadRecord(bad.clone()).into());
        }
        let value = apply(&mut Tx::new(&mut doc))?;
        let path = self.path();
        cw_fs::write_atomic(&path, &format::encode(&doc)?).map_err(Error::from)?;
        Ok(value)
    }

    /// The audit log's repository.
    #[must_use]
    pub fn audit(&self) -> Audit<'_> {
        Audit::new(self)
    }

    pub(crate) fn lock(&self, label: &str) -> Result<Lock> {
        Ok(Lock::acquire(
            &self.root.join(LOCK_FILE),
            label,
            self.lock_wait,
        )?)
    }

    pub(crate) fn audit_lock(&self, label: &str) -> Result<Lock> {
        Ok(Lock::acquire(
            &self.root.join(AUDIT_LOCK_FILE),
            label,
            self.lock_wait,
        )?)
    }

    fn load(&self) -> Result<StoreDocument> {
        let path = self.path();
        if !cw_fs::inspect_dir(&self.root)? || !cw_fs::inspect_file(&path)? {
            return Ok(StoreDocument::empty());
        }
        let bytes = cw_fs::read(&path, STORE_CAP)?;
        format::decode(&path, &bytes)
    }
}
