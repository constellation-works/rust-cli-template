//! The notes database's schema: one module per table, each exporting that
//! table's steps as named items, and here the one ordered list of every step
//! handed to `cw_sqlite`. A statement that changes the schema appears only
//! under this directory (`scripts/check-dependency-direction.sh`).
//!
//! Conventions, from the kit's example: `STRICT` tables, timestamps as
//! RFC 3339 UTC text, a list as JSON in a text column checked by
//! `json_valid`.

use cw_sqlite::Step;

mod note;

/// Every schema step, append-only (STD-03 R23): step `n` takes the database
/// from version `n - 1` to `n` (`PRAGMA user_version`). A shipped step is
/// never edited, reordered or removed, so an older build refuses a newer
/// database instead of misreading it (STD-03 R10).
///
/// Each step is named here, never spliced in as another table's list: a step
/// added later to an earlier table goes at the end, after every shipped
/// step, and no shipped step is renumbered.
pub(crate) const MIGRATIONS: &[Step] = &[
    // 1
    note::CREATE_NOTE,
];
