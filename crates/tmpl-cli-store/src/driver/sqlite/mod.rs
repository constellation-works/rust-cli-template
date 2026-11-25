//! The notes database: where it lives, how long a call waits for it, and its
//! schema, handed out as one `cw_sqlite::Db`.
//!
//! The kit's `cw_sqlite` is the mechanism (private files, WAL, one
//! `IMMEDIATE` write transaction, a read-only reader, versioned migrations);
//! this module holds only what is this project's: the file name and the
//! schema steps. It runs no query: the notes' SQL and row mapping live in
//! `crate::repository::notes`. A statement that changes the schema appears
//! only here (`scripts/check-dependency-direction.sh`).

use cw_sqlite::{Db, Step};
use std::path::Path;
use std::time::Duration;

/// The notes database's file name under the data directory. SQLite keeps
/// its `-wal` and `-shm` files beside it.
pub const STORE_FILE: &str = "notes.db";

/// The schema, append-only (STD-03 R23): step `n` takes the database from
/// version `n - 1` to `n` (`PRAGMA user_version`). A shipped step is never
/// edited, reordered or removed; a change is a new step, so an older build
/// refuses a newer database instead of misreading it (STD-03 R10).
///
/// Conventions, from the kit's example: `STRICT` tables, timestamps as
/// RFC 3339 UTC text, a list as JSON in a text column checked by
/// `json_valid`. Each column holds what the contract types hold; a value a
/// contract refuses (an invalid title, tag or priority, an unparseable
/// timestamp) is isolated as a bad record when it is read, never defaulted.
pub(crate) const MIGRATIONS: &[Step] = &[Step::sql(
    "CREATE TABLE note (
        id         INTEGER PRIMARY KEY CHECK (id > 0),
        title      TEXT NOT NULL,
        body       TEXT,
        tags       TEXT NOT NULL CHECK (json_valid(tags) AND json_type(tags) = 'array'),
        priority   TEXT NOT NULL,
        created_at TEXT NOT NULL
    ) STRICT;",
)];

/// The notes database under `root`, waiting up to `lock_wait` for another
/// connection. Opens nothing: each read or write opens its own connection.
pub(crate) fn open(root: &Path, lock_wait: Duration) -> Db {
    Db::new(root.join(STORE_FILE), MIGRATIONS).lock_wait(lock_wait)
}
