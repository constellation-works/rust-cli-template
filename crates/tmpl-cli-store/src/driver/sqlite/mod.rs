//! The notes database: where it lives and how long a call waits for it,
//! handed out as one `cw_sqlite::Db` over the steps in [`schema`].
//!
//! The kit's `cw_sqlite` is the mechanism (private files, WAL, one
//! `IMMEDIATE` write transaction, a read-only reader, versioned migrations);
//! this driver holds only what is this project's: the file name here and the
//! schema steps in `schema/`. It runs no query: each record's SQL and row
//! mapping live in its repository (`crate::repository::notes`).

use cw_sqlite::Db;
use std::path::Path;
use std::time::Duration;

mod schema;

/// The notes database's file name under the data directory. SQLite keeps
/// its `-wal` and `-shm` files beside it.
pub const STORE_FILE: &str = "notes.db";

/// The notes database under `root`, waiting up to `lock_wait` for another
/// connection. Opens nothing: each read or write opens its own connection.
pub(crate) fn open(root: &Path, lock_wait: Duration) -> Db {
    Db::new(root.join(STORE_FILE), schema::MIGRATIONS).lock_wait(lock_wait)
}
