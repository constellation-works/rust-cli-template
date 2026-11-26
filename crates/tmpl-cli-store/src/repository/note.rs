//! The notes repository: note records by id, as rows of the `note` table.
//!
//! This module holds the notes' SQL and the mapping between a row and the
//! contract shapes; the database itself (its file, schema and connections)
//! is the driver's (`crate::driver::sqlite`). It runs on the reader that
//! [`crate::Store::read`] opened or the write transaction that
//! [`crate::Store::write`] began, and returns owned values: nothing borrowed
//! from a connection or a row leaves it.

use crate::contract::{Note, NoteId, Priority, Tag, Title};
use crate::error::{BadRecord, BadRecordReason, Error, RecordLocator, Result};
use cw_sqlite::rusqlite::types::ValueRef;
use cw_sqlite::rusqlite::{Connection, OptionalExtension as _, Params, Row, params};
use std::path::Path;
use std::str::FromStr;
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

/// Every column of a note, in the order [`StoredRow::read`] reads them.
const COLUMNS: &str = "id, title, body, tags, priority, created_at";

/// Note records as one read saw them. A store that does not exist yet has
/// no connection and reads as empty.
#[derive(Clone, Copy, Debug)]
pub struct Notes<'a> {
    conn: Option<&'a Connection>,
    path: &'a Path,
}

impl<'a> Notes<'a> {
    pub(crate) fn new(conn: Option<&'a Connection>, path: &'a Path) -> Self {
        Self { conn, path }
    }

    /// Every valid note, oldest (lowest id) first.
    ///
    /// # Errors
    ///
    /// The store's typed `Error` when the query fails, translated into
    /// `cw_base::Error`.
    pub fn all(&self) -> std::result::Result<Vec<Note>, cw_base::Error> {
        let Some(conn) = self.conn else {
            return Ok(Vec::new());
        };
        Ok(valid(all_rows(conn, self.path)?))
    }

    /// The valid note with this id; `None` when there is none, or when the
    /// record with this id is a bad one ([`Notes::bad_record`]).
    ///
    /// # Errors
    ///
    /// As for [`Notes::all`].
    pub fn get(&self, id: NoteId) -> std::result::Result<Option<Note>, cw_base::Error> {
        let Some(conn) = self.conn else {
            return Ok(None);
        };
        Ok(one_row(conn, self.path, id)?.and_then(std::result::Result::ok))
    }

    /// Every note record that failed validation, isolated from the valid
    /// ones (STD-02 R32); empty when every record was valid.
    ///
    /// # Errors
    ///
    /// As for [`Notes::all`].
    pub fn bad_records(&self) -> std::result::Result<Vec<BadRecord>, cw_base::Error> {
        let Some(conn) = self.conn else {
            return Ok(Vec::new());
        };
        Ok(bad(all_rows(conn, self.path)?))
    }

    /// The bad record with this id.
    ///
    /// # Errors
    ///
    /// As for [`Notes::all`].
    pub fn bad_record(&self, id: NoteId) -> std::result::Result<Option<BadRecord>, cw_base::Error> {
        let Some(conn) = self.conn else {
            return Ok(None);
        };
        Ok(one_row(conn, self.path, id)?.and_then(std::result::Result::err))
    }
}

/// Note records inside one write.
#[derive(Debug)]
pub struct NotesMut<'a> {
    conn: &'a Connection,
    path: &'a Path,
}

impl<'a> NotesMut<'a> {
    pub(crate) fn new(conn: &'a Connection, path: &'a Path) -> Self {
        Self { conn, path }
    }

    /// Every valid note, oldest (lowest id) first, as this write sees it.
    ///
    /// # Errors
    ///
    /// As for [`Notes::all`].
    pub fn all(&self) -> std::result::Result<Vec<Note>, cw_base::Error> {
        Ok(valid(all_rows(self.conn, self.path)?))
    }

    /// Store `note`: replace the note with its id, or add it when the id is
    /// new.
    ///
    /// # Errors
    ///
    /// The store's typed `Error` when the note cannot be encoded or the
    /// statement fails, translated into `cw_base::Error`.
    pub fn put(&mut self, note: &Note) -> std::result::Result<(), cw_base::Error> {
        Ok(put(self.conn, self.path, note)?)
    }
}

/// The first bad record in the store, for a write's refusal.
pub(crate) fn first_bad(conn: &Connection, path: &Path) -> Result<Option<BadRecord>> {
    Ok(bad(all_rows(conn, path)?).into_iter().next())
}

fn valid(rows: Vec<std::result::Result<Note, BadRecord>>) -> Vec<Note> {
    rows.into_iter()
        .filter_map(std::result::Result::ok)
        .collect()
}

fn bad(rows: Vec<std::result::Result<Note, BadRecord>>) -> Vec<BadRecord> {
    rows.into_iter()
        .filter_map(std::result::Result::err)
        .collect()
}

/// Every row, each a note or the bad record it was isolated as.
fn all_rows(conn: &Connection, path: &Path) -> Result<Vec<std::result::Result<Note, BadRecord>>> {
    query(
        conn,
        path,
        &format!("SELECT {COLUMNS} FROM note ORDER BY id"),
        (),
    )
}

/// The row with this id, if any.
fn one_row(
    conn: &Connection,
    path: &Path,
    id: NoteId,
) -> Result<Option<std::result::Result<Note, BadRecord>>> {
    let sql = format!("SELECT {COLUMNS} FROM note WHERE id = ?1");
    let action = "read a note from";
    let id = sql_id(id).map_err(|err| sqlite(action, path, err))?;
    let mut statement = conn
        .prepare_cached(&sql)
        .map_err(|err| sqlite(action, path, err))?;
    let row = statement
        .query_row(params![id], StoredRow::read)
        .optional()
        .map_err(|err| sqlite(action, path, err))?;
    Ok(row.map(|row| row.into_note(path)))
}

fn query(
    conn: &Connection,
    path: &Path,
    sql: &str,
    params: impl Params,
) -> Result<Vec<std::result::Result<Note, BadRecord>>> {
    let action = "read the notes in";
    let mut statement = conn
        .prepare_cached(sql)
        .map_err(|err| sqlite(action, path, err))?;
    let rows = statement
        .query_map(params, StoredRow::read)
        .map_err(|err| sqlite(action, path, err))?;
    rows.map(|row| {
        row.map(|row| row.into_note(path))
            .map_err(|err| sqlite(action, path, err))
    })
    .collect()
}

fn put(conn: &Connection, path: &Path, note: &Note) -> Result<()> {
    let tags: Vec<&str> = note.tags.iter().map(Tag::as_str).collect();
    let tags = serde_json::to_string(&tags).map_err(|source| Error::Encode {
        what: "note tags",
        source,
    })?;
    let created_at = note
        .created_at
        .to_offset(UtcOffset::UTC)
        .format(&Rfc3339)
        .map_err(|source| Error::Timestamp { source })?;
    let action = "store a note in";
    conn.prepare_cached(
        "INSERT INTO note (id, title, body, tags, priority, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (id) DO UPDATE SET
            title = excluded.title,
            body = excluded.body,
            tags = excluded.tags,
            priority = excluded.priority,
            created_at = excluded.created_at",
    )
    .and_then(|mut statement| {
        statement.execute(params![
            sql_id(note.id)?,
            note.title.as_str(),
            note.body.as_deref(),
            tags,
            note.priority.as_str(),
            created_at,
        ])
    })
    .map_err(|err| sqlite(action, path, err))?;
    Ok(())
}

/// A note id as SQLite's integer. [`NoteId`] never exceeds `i64::MAX`, so
/// this conversion always succeeds.
fn sql_id(id: NoteId) -> cw_sqlite::rusqlite::Result<i64> {
    i64::try_from(id.get())
        .map_err(|err| cw_sqlite::rusqlite::Error::ToSqlConversionFailure(Box::new(err)))
}

fn sqlite(action: &'static str, path: &Path, err: cw_sqlite::rusqlite::Error) -> Error {
    Error::Sqlite(cw_sqlite::Error::sqlite(action, path, err))
}

/// One row as SQLite holds it, before UTF-8 decoding and the contract types
/// check it. Text columns are read as raw bytes so malformed UTF-8 in one row
/// becomes a [`BadRecord`] rather than aborting the query (STD-02 R32).
struct StoredRow {
    id: i64,
    title: Vec<u8>,
    body: Option<Vec<u8>>,
    tags: Vec<u8>,
    priority: Vec<u8>,
    created_at: Vec<u8>,
}

impl StoredRow {
    fn read(row: &Row<'_>) -> cw_sqlite::rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            title: raw_text(row, 1, "title")?,
            body: raw_opt_text(row, 2, "body")?,
            tags: raw_text(row, 3, "tags")?,
            priority: raw_text(row, 4, "priority")?,
            created_at: raw_text(row, 5, "created_at")?,
        })
    }

    /// The note this row holds, or the bad record it is isolated as: each
    /// field is decoded and validated at the construction boundary, with no
    /// default (STD-02 R32).
    fn into_note(self, path: &Path) -> std::result::Result<Note, BadRecord> {
        let id = u64::try_from(self.id)
            .map_err(|_| crate::contract::NoteError::InvalidId(self.id.to_string()))
            .and_then(NoteId::try_from);
        let id = match id {
            Ok(id) => id,
            Err(err) => {
                return Err(BadRecord {
                    path: path.to_path_buf(),
                    locator: RecordLocator::Row(self.id),
                    reason: BadRecordReason::Validation(err),
                });
            }
        };
        let bad = |reason| BadRecord {
            path: path.to_path_buf(),
            locator: RecordLocator::Id(id),
            reason,
        };
        let title_text = decode_utf8("title", self.title).map_err(&bad)?;
        let title =
            Title::try_from(title_text).map_err(|err| bad(BadRecordReason::Validation(err)))?;
        let body = self
            .body
            .map(|bytes| decode_utf8("body", bytes))
            .transpose()
            .map_err(&bad)?;
        let tags_text = decode_utf8("tags", self.tags).map_err(&bad)?;
        let tags = parse_tags(&tags_text).map_err(&bad)?;
        let priority_text = decode_utf8("priority", self.priority).map_err(&bad)?;
        let priority = Priority::from_str(&priority_text)
            .map_err(|err| bad(BadRecordReason::Validation(err)))?;
        let created_at_text = decode_utf8("created_at", self.created_at).map_err(&bad)?;
        let created_at = OffsetDateTime::parse(&created_at_text, &Rfc3339).map_err(|err| {
            bad(BadRecordReason::InvalidField {
                field: "created_at",
                detail: err.to_string(),
            })
        })?;
        Ok(Note {
            id,
            title,
            body,
            tags,
            priority,
            created_at,
        })
    }
}

fn raw_text(row: &Row<'_>, index: usize, name: &str) -> cw_sqlite::rusqlite::Result<Vec<u8>> {
    match row.get_ref(index)? {
        ValueRef::Text(bytes) => Ok(bytes.to_vec()),
        other @ (ValueRef::Null | ValueRef::Integer(_) | ValueRef::Real(_) | ValueRef::Blob(_)) => {
            Err(cw_sqlite::rusqlite::Error::InvalidColumnType(
                index,
                name.to_owned(),
                other.data_type(),
            ))
        }
    }
}

fn raw_opt_text(
    row: &Row<'_>,
    index: usize,
    name: &str,
) -> cw_sqlite::rusqlite::Result<Option<Vec<u8>>> {
    match row.get_ref(index)? {
        ValueRef::Null => Ok(None),
        ValueRef::Text(bytes) => Ok(Some(bytes.to_vec())),
        other @ (ValueRef::Integer(_) | ValueRef::Real(_) | ValueRef::Blob(_)) => {
            Err(cw_sqlite::rusqlite::Error::InvalidColumnType(
                index,
                name.to_owned(),
                other.data_type(),
            ))
        }
    }
}

fn decode_utf8(
    field: &'static str,
    bytes: Vec<u8>,
) -> std::result::Result<String, BadRecordReason> {
    String::from_utf8(bytes).map_err(|err| BadRecordReason::InvalidField {
        field,
        detail: format!("invalid UTF-8: {}", err.utf8_error()),
    })
}

fn parse_tags(text: &str) -> std::result::Result<Vec<Tag>, BadRecordReason> {
    let items: Vec<String> =
        serde_json::from_str(text).map_err(|_| BadRecordReason::InvalidField {
            field: "tags",
            detail: String::from("expected an array of strings"),
        })?;
    items
        .into_iter()
        .map(|item| Tag::try_from(item).map_err(BadRecordReason::Validation))
        .collect()
}
