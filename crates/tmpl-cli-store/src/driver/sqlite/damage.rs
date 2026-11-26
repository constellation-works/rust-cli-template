//! Telling a database that is not one from one that only lacks its log files.
//!
//! The kit's strict read looks for the `-wal` and `-shm` first, so a file that
//! is not a SQLite database and has neither is reported as lacking them. The
//! store must report it as damaged (`store_corrupt`), so on that refusal it
//! looks at the main file once more, the one way that creates and changes
//! nothing: read-only, `immutable=1` (no log, no index, no lock).

use cw_sqlite::rusqlite::{Connection, OpenFlags};
use std::path::Path;

/// The kit's `Corrupt` error for the database at `path` when SQLite finds it
/// is not a database or is damaged, read on its own, without its log files.
/// `None` when it reads as a database, or when it cannot be looked at.
pub(crate) fn find(path: &Path) -> Option<cw_sqlite::Error> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI;
    let conn = Connection::open_with_flags(immutable_uri(path)?, flags).ok()?;
    let read = conn.query_row("SELECT count(*) FROM sqlite_schema", [], |row| {
        row.get::<_, i64>(0)
    });
    let err = cw_sqlite::Error::sqlite("read", path, read.err()?);
    matches!(err, cw_sqlite::Error::Corrupt { .. }).then_some(err)
}

/// The `file:` URI that opens `path` immutably. `None` for a path that is
/// not UTF-8, which a URI cannot carry.
fn immutable_uri(path: &Path) -> Option<String> {
    let mut uri = String::from("file:");
    for ch in path.to_str()?.chars() {
        match ch {
            '%' => uri.push_str("%25"),
            '?' => uri.push_str("%3f"),
            '#' => uri.push_str("%23"),
            other => uri.push(other),
        }
    }
    uri.push_str("?immutable=1");
    Some(uri)
}
