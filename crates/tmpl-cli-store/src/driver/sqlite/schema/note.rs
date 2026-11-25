//! The `note` table's schema steps, one named item per step. Each is listed
//! once, in order, in `super::MIGRATIONS`; a step defined here is never
//! edited after it ships.

use cw_sqlite::Step;

/// Version 1: the `note` table.
///
/// Each column holds what the contract types hold; a value a contract
/// refuses (an invalid title, tag or priority, an unparseable timestamp) is
/// isolated as a bad record when it is read, never defaulted.
pub(crate) const CREATE_NOTE: Step = Step::sql(
    "CREATE TABLE note (
        id         INTEGER PRIMARY KEY CHECK (id > 0),
        title      TEXT NOT NULL,
        body       TEXT,
        tags       TEXT NOT NULL CHECK (json_valid(tags) AND json_type(tags) = 'array'),
        priority   TEXT NOT NULL,
        created_at TEXT NOT NULL
    ) STRICT;",
);
