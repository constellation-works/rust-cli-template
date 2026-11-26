//! The database: where it lives and how long a call waits for it,
//! handed out as one `cw_sqlite::Db` over the steps in [`schema`].
//!
//! The kit's `cw_sqlite` is the mechanism (private files, WAL, one
//! `IMMEDIATE` write transaction, a read-only reader, versioned migrations);
//! this driver holds only what is this project's: the file name here and the
//! schema steps in `schema/`. It runs no record's query: each record's SQL and
//! row mapping live in its repository under `crate::repository`; the one read
//! it makes is the damage probe in `damage`.

use cw_sqlite::Db;
pub(crate) use damage::find as find_damage;
use std::path::Path;
use std::time::Duration;

mod damage;

mod schema;

/// The database's file name under the data directory. SQLite keeps
/// its `-wal` and `-shm` files beside it.
pub const STORE_FILE: &str = "store.db";

/// The database under `root`, waiting up to `lock_wait` for another
/// connection. Opens nothing: each read or write opens its own connection.
pub(crate) fn open(root: &Path, lock_wait: Duration) -> Db {
    Db::new(root.join(STORE_FILE), schema::MIGRATIONS).lock_wait(lock_wait)
}
