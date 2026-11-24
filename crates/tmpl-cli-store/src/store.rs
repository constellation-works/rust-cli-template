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
    /// The most bytes the store file may hold, read or written.
    cap: cw_fs::Cap,
}

impl Store {
    /// A store at `root`. Performs no I/O; the directory is created by the
    /// first write. The caller resolves `root` (STD-02 R3): this crate never
    /// reads the environment or the home directory.
    pub fn open(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            lock_wait: DEFAULT_LOCK_WAIT,
            cap: STORE_CAP,
        }
    }

    /// A smaller size limit, so a test can reach it without writing 64 MiB.
    #[cfg(test)]
    pub(crate) fn with_cap(mut self, cap: cw_fs::Cap) -> Self {
        self.cap = cap;
        self
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
    ///
    /// # Errors
    ///
    /// The store's typed [`Error`], translated into `cw_base::Error` here at
    /// the crate's boundary; `downcast_ref::<Error>()` gives it back.
    pub fn read(&self) -> std::result::Result<Snapshot, cw_base::Error> {
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
    ///
    /// # Errors
    ///
    /// `apply`'s own error, or the store's typed [`Error`] translated into
    /// `cw_base::Error` here at the crate's boundary, which `E` converts
    /// from.
    pub fn write<T, E>(
        &self,
        label: &str,
        apply: impl FnOnce(&mut Tx<'_>) -> std::result::Result<T, E>,
    ) -> std::result::Result<T, E>
    where
        E: From<cw_base::Error>,
    {
        let (_lock, mut doc) = self.begin(label).map_err(cw_base::Error::from)?;
        let value = apply(&mut Tx::new(&mut doc))?;
        self.commit(&doc).map_err(cw_base::Error::from)?;
        Ok(value)
    }

    /// Everything a write does before `apply`: create the data directory,
    /// take the lock, and re-read the store under it, refusing bad records.
    fn begin(&self, label: &str) -> Result<(Lock, StoreDocument)> {
        cw_fs::create_private_dir(&self.root)?;
        let lock = self.lock(label)?;
        // Re-read under the lock: anything read before it may be stale.
        let doc = self.load()?;
        if let Some(bad) = doc.bad_records.first() {
            return Err(Error::BadRecord(bad.clone()));
        }
        Ok((lock, doc))
    }

    /// Replace the store file with `doc`, unless it would be larger than
    /// reads accept: a write must never leave a store its own reads refuse.
    fn commit(&self, doc: &StoreDocument) -> Result<()> {
        let path = self.path();
        let bytes = format::encode(doc)?;
        let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if size > self.cap.get() {
            return Err(Error::WouldExceedCap {
                path,
                size,
                cap: self.cap.get(),
            });
        }
        cw_fs::write_atomic(&path, &bytes)?;
        Ok(())
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
        let bytes = cw_fs::read(&path, self.cap)?;
        format::decode(&path, &bytes)
    }
}
