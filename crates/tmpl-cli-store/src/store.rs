//! Opens the store and coordinates reads and writes; imports driver, error and repository.
//!
//! The [`Store`] facade: one data directory, read as a snapshot, written in
//! one transaction.
//!
//! The records live in one SQLite database, `<root>/store.db`, with its `-wal`
//! and `-shm` beside it, set up by the driver (`crate::driver::sqlite`) and
//! reached through the kit's `cw_sqlite`. An observation reads strictly
//! ([`Store::read`]): a read-only connection whose index is read-only too, so
//! it creates, rebuilds and changes no file (STD-01 R31) and sees one
//! committed version; a database whose `-wal` or `-shm` is missing is refused
//! (`store_wal_unavailable`) rather than read around, and the next write
//! creates them. A write runs in one `IMMEDIATE`
//! transaction, so two concurrent writers never lose each other's records
//! (STD-03 R5, R6); a writer waits for another up to the lock wait. The
//! audit log in the same directory is the kit's (`cw_app`), not the
//! store's. New directories are created `0700` and new files `0600`
//! (STD-05 R8), and every read and write refuses a data directory or file
//! that is a symbolic link, owned by another user, or open to any group or
//! other permission bit (STD-05 R9).
//!
//! The store decides nothing about a record: which id comes next, which
//! records a listing returns and when a command is refused are an app's rules.

use crate::driver::sqlite::{self, STORE_FILE};
use crate::error::{Error, Result};
use crate::repository::{Snapshot, Tx};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long a writer waits for another writer before refusing (STD-03 R7).
const DEFAULT_LOCK_WAIT: Duration = Duration::from_secs(10);

/// A store rooted at one data directory.
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

    /// Override how long a writer waits for another connection.
    #[must_use]
    pub fn with_lock_wait(mut self, wait: Duration) -> Self {
        self.lock_wait = wait;
        self
    }

    /// The database, `<root>/store.db`. Public for the apps, which name it
    /// in an outcome and in their refusals.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.root.join(STORE_FILE)
    }

    /// The data directory. Public for the apps, which report it.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Everything the store holds, as one strict read-only connection sees
    /// it: for a command that observes and writes nothing (STD-01 R31). A
    /// missing data directory or database reads as an empty store. Takes no
    /// lock and creates, rebuilds or changes no file, not even a `-wal` or
    /// `-shm`.
    ///
    /// A database whose `-wal` or `-shm` is missing is refused with
    /// `store_wal_unavailable`: in a writable directory a writer may exist, so
    /// the main file alone is not trusted. The next write
    /// creates the files, after which this read works. The kit's writers keep
    /// both files after their last close, so a store this template wrote is
    /// readable until another tool removes them.
    ///
    /// Drop the snapshot before writing in the same process: SQLite maps the
    /// index once per process, so a write while a strict reader of the same
    /// database is open fails with `store_sqlite_error`.
    ///
    /// # Errors
    ///
    /// The store's typed [`Error`], translated into `cw_base::Error` here at
    /// the crate's boundary; `downcast_ref::<Error>()` gives it back.
    ///
    /// ```
    /// let temp = cw_testkit::TempRoot::new("TMPL_CLI")?;
    /// let store = tmpl_cli_store::Store::open(temp.path().join("data"));
    /// let snapshot = store.read()?;
    /// assert_eq!(snapshot.format(), None, "no database yet");
    /// assert!(!store.root().exists(), "a read creates nothing");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn read(&self) -> std::result::Result<Snapshot, cw_base::Error> {
        Ok(self.snapshot(Mode::Strict)?)
    }

    /// The same view as [`Store::read`], for the checks a write makes
    /// before it begins (STD-02 R34). It is an ordinary read: in a writable
    /// directory SQLite may create a missing `-wal` or `-shm` or rebuild the
    /// `-shm`, and it never refuses a database for lacking them, so a write
    /// can repair a store whose observation is refused. Only a command that
    /// writes may use it.
    ///
    /// # Errors
    ///
    /// As [`Store::read`], except `store_wal_unavailable`.
    pub fn read_before_write(&self) -> std::result::Result<Snapshot, cw_base::Error> {
        Ok(self.snapshot(Mode::Ordinary)?)
    }

    /// Change the store in one transaction: check the data directory and
    /// create it if needed, open the database (creating and migrating it
    /// when needed), begin an `IMMEDIATE` write transaction, run `apply`
    /// against it, and commit if `apply` succeeds. Nothing is written when
    /// `apply` fails: the transaction is rolled back.
    ///
    /// A store holding bad records is refused (`store_bad_record`) before
    /// `apply` runs, so a bad record is repaired before anything is added
    /// beside it (STD-02 R31). Checks a caller wants made before the
    /// directory or the database is created belong in its own read before
    /// this call (STD-02 R34).
    ///
    /// `apply` returns the workspace's error type too: a caller's own
    /// refusal inside the transaction crosses the store's boundary already
    /// translated, so nothing of another crate's typed error passes through
    /// the store.
    ///
    /// # Errors
    ///
    /// `apply`'s error as it returned it, or the store's typed [`Error`]
    /// translated into `cw_base::Error` here at the crate's boundary.
    pub fn write<T>(
        &self,
        apply: impl FnOnce(&mut Tx<'_>) -> std::result::Result<T, cw_base::Error>,
    ) -> std::result::Result<T, cw_base::Error> {
        self.prepare_root()?;
        let db = sqlite::open(&self.root, self.lock_wait);
        let path = db.path();
        db.write(|tx| {
            // example:begin
            if let Some(bad) = crate::repository::note::first_bad(tx, path)? {
                return Err(Failed::from(Error::BadRecord(bad)));
            }
            // example:end
            apply(&mut Tx::new(tx, path)).map_err(Failed)
        })
        .map_err(|Failed(err)| err)
    }

    /// Check the data directory (a link, another user's, open to others)
    /// before anything is created in it, so a refused root gains nothing
    /// (STD-02 R34), and create it `0700` when absent.
    pub(crate) fn prepare_root(&self) -> Result<()> {
        if !cw_fs::inspect_dir(&self.root)? {
            cw_fs::create_private_dir(&self.root)?;
        }
        Ok(())
    }

    fn snapshot(&self, mode: Mode) -> Result<Snapshot> {
        let db = sqlite::open(&self.root, self.lock_wait);
        let path = db.path().to_path_buf();

        if !cw_fs::inspect_dir(&self.root)? {
            return Ok(Snapshot::new(None, path));
        }

        let reader = match mode {
            Mode::Strict => db.read_strict(),
            Mode::Ordinary => db.read(),
        };

        match reader {
            Ok(reader) => Ok(Snapshot::new(Some(reader), path)),
            Err(cw_sqlite::Error::Missing { .. }) => Ok(Snapshot::new(None, path)),
            // The kit asks for the log files before it reads a page, so a file
            // that is not a database is reported as lacking them: say which.
            Err(err @ cw_sqlite::Error::WalUnavailable { .. }) => {
                Err(sqlite::find_damage(&path).unwrap_or(err).into())
            }
            Err(err) => Err(err.into()),
        }
    }
}

/// How a snapshot is opened.
#[derive(Clone, Copy)]
enum Mode {
    /// Creates and changes no file; refuses a database lacking its `-wal` or
    /// `-shm`.
    Strict,
    /// May create or rebuild the `-wal` and `-shm`; for a write's own checks.
    Ordinary,
}

/// A write's failure, whichever side it came from: the kit's database
/// error (converted by `cw_sqlite::Db::write`) or the workspace's error that
/// `apply` or the store's own refusal returned.
struct Failed(cw_base::Error);

impl From<Error> for Failed {
    fn from(err: Error) -> Self {
        Self(err.into())
    }
}

impl From<cw_sqlite::Error> for Failed {
    fn from(err: cw_sqlite::Error) -> Self {
        Self::from(Error::Sqlite(err))
    }
}
