//! Hand-made store state for tests, behind the `test-support` feature.
//!
//! A test that needs a store the public API never writes (a bad record, an
//! id at the end of the range, a database from a newer build) runs raw SQL
//! on the database here, so no crate outside this one names
//! `rusqlite`. Production builds never enable the feature.

use crate::Store;
use crate::error::Error;

/// Run `sql` (one or more statements) in one write transaction on the
/// database of `store`, creating the data directory and the database, at
/// the current schema, when they are absent.
///
/// # Errors
///
/// The store's typed `Error`, translated into `cw_base::Error`.
pub fn execute(store: &Store, sql: &str) -> std::result::Result<(), cw_base::Error> {
    store.prepare_root()?;
    let db = crate::driver::sqlite::open(store.root(), std::time::Duration::from_secs(5));
    db.write(|tx| {
        tx.execute_batch(sql).map_err(|err| {
            Error::Sqlite(cw_sqlite::Error::sqlite("run test SQL on", db.path(), err))
        })
    })
    .map_err(cw_base::Error::from)
}

/// Fold the database's write-ahead log into the main file and truncate it,
/// so the main file alone holds every committed record, as a copy of just
/// that file would. Does nothing when there is no database yet.
///
/// # Errors
///
/// The store's typed `Error`, translated into `cw_base::Error`.
pub fn checkpoint(store: &Store) -> std::result::Result<(), cw_base::Error> {
    let db = crate::driver::sqlite::open(store.root(), std::time::Duration::from_secs(5));
    db.checkpoint_truncate()
        .map_err(|err| cw_base::Error::from(Error::Sqlite(err)))
}
