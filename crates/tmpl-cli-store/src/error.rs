//! The store's one error type, and the bad-record report a read carries.
//!
//! Callers branch on the variant (or on [`Error::code`]), never on the message
//! (STD-02 R10). `Display` carries only this layer's sentence; the underlying
//! cause stays on [`std::error::Error::source`], so a surface can print the
//! whole chain once without repeating it. The one translator into the
//! workspace's error type is the `From` impl at the end of this file
//! (STD-02 R12).

use crate::contract::{NoteError, NoteId};
use cw_base::Kind;
use std::fmt;
use std::path::PathBuf;

/// Where a bad record sits in the notes database: its note id when it is a
/// valid one, otherwise the row's raw id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordLocator {
    /// The record had a valid note id.
    Id(NoteId),
    /// The row's id is not a valid note id; identified by the raw value.
    Row(i64),
}

impl fmt::Display for RecordLocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Id(id) => write!(f, "note {id}"),
            Self::Row(row) => write!(f, "row {row}"),
        }
    }
}

/// Why a stored note record failed validation.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BadRecordReason {
    /// A field held a value that does not parse: a timestamp that is not
    /// RFC 3339, or tags that are not a JSON array of strings.
    #[error("invalid field '{field}': {detail}")]
    InvalidField {
        /// The offending field name.
        field: &'static str,
        /// What was wrong with its value.
        detail: String,
    },
    /// A domain validation rule rejected a field value.
    #[error(transparent)]
    Validation(#[from] NoteError),
}

/// One invalid note record isolated while loading a store (STD-02 R32).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("bad record at {locator} in {}: {reason}", path.display())]
pub struct BadRecord {
    /// The notes database holding the record.
    pub path: PathBuf,
    /// The record's note id, or the row's raw id when that is not one.
    pub locator: RecordLocator,
    /// Why the record was rejected.
    pub reason: BadRecordReason,
}

impl BadRecord {
    /// This record as the store's refusal (`store_bad_record`), in the
    /// workspace's error type: what an app returns when a command would
    /// read or rewrite it.
    #[must_use]
    pub fn refusal(&self) -> cw_base::Error {
        Error::BadRecord(self.clone()).into()
    }
}

/// Everything a [`crate::Store`] operation can fail with. Failures of an
/// app's own rules (an unknown id, an exhausted id space) are the app's
/// errors in `tmpl-cli-core`, not variants here.
///
/// It never leaves the crate as itself: every public operation returns
/// `cw_base::Error`, translated by the `From` impl below, and a caller that
/// needs a variant gets it back with `downcast_ref::<Error>()`.
/// `#[non_exhaustive]`, so adding a variant is not a breaking change.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A file operation on the data directory or the audit log failed, was
    /// refused, or timed out on the audit lock. The kit's error carries a
    /// kind and no code: the store names the code ([`Error::code`]).
    #[error(transparent)]
    Fs(#[from] cw_fs::Error),
    /// Opening, migrating, reading or writing the notes database failed or
    /// was refused. The kit's error carries a kind and no code: the store
    /// names the code ([`Error::code`]).
    #[error(transparent)]
    Sqlite(#[from] cw_sqlite::Error),
    /// Serializing a note's tags or an audit event failed before anything was written.
    #[error("cannot encode {what}")]
    Encode {
        /// What was being serialized.
        what: &'static str,
        /// What the serializer rejected.
        #[source]
        source: serde_json::Error,
    },
    /// A persisted file (the audit log) is not one this build can parse.
    #[error("{} is not a valid store file", path.display())]
    Corrupt {
        /// The file.
        path: PathBuf,
        /// What the parser rejected.
        #[source]
        source: serde_json::Error,
    },
    /// One or more note records in the store failed validation.
    #[error(transparent)]
    BadRecord(#[from] BadRecord),
    /// A note's timestamp could not be formatted as RFC 3339 before anything
    /// was written.
    #[error("cannot format a note's timestamp as RFC 3339")]
    Timestamp {
        /// What the formatter rejected.
        #[source]
        source: time::error::Format,
    },
    /// A persisted file (the audit log) was written by a newer build
    /// (STD-03 R10: refuse, never rewrite or reinterpret newer state). A
    /// newer notes database is the kit's `Incompatible`.
    #[error(
        "{} uses store format {found}, but this build reads format {supported} or older; upgrade this tool",
        path.display()
    )]
    NewerFormat {
        /// The file.
        path: PathBuf,
        /// The format the file declares.
        found: u32,
        /// The newest format this build understands.
        supported: u32,
    },
}

impl Error {
    /// A stable `snake_case` code for this failure, for machine output
    /// (STD-01 R19). Adding a code is fine; changing one is a breaking change.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            // The kit's file failures get the store's codes here.
            Self::Fs(e) | Self::Sqlite(cw_sqlite::Error::File(e)) => fs_code(e),
            // The kit's database failures get the store's codes here.
            Self::Sqlite(cw_sqlite::Error::Missing { .. }) => "store_missing",
            Self::Sqlite(cw_sqlite::Error::Locked { .. }) => "store_locked",
            Self::Sqlite(cw_sqlite::Error::Corrupt { .. }) | Self::Corrupt { .. } => {
                "store_corrupt"
            }
            Self::Sqlite(cw_sqlite::Error::CheckFailed { .. }) => "store_check_failed",
            Self::Sqlite(cw_sqlite::Error::Incompatible { .. }) | Self::NewerFormat { .. } => {
                "store_too_new"
            }
            Self::Sqlite(cw_sqlite::Error::NotWal { .. }) => "store_not_wal",
            Self::Sqlite(cw_sqlite::Error::Migration { .. }) => "store_migration_failed",
            Self::Sqlite(cw_sqlite::Error::Sqlite { .. }) => "store_sqlite_error",
            Self::Encode { .. } | Self::Timestamp { .. } => "encode_error",
            Self::BadRecord(_) => "store_bad_record",
        }
    }

    /// How this failure is classified, for the workspace's error type.
    /// Exhaustive, so a new variant decides (STD-02 R27).
    #[must_use]
    pub fn kind(&self) -> Kind {
        match self {
            Self::Fs(e) => e.kind(),
            Self::Sqlite(e) => e.kind(),
            Self::Encode { .. } | Self::Timestamp { .. } => Kind::Internal,
            Self::Corrupt { .. } | Self::BadRecord(_) => Kind::Corrupt,
            Self::NewerFormat { .. } => Kind::Incompatible,
        }
    }
}

/// The store's code for each of the kit's file errors, met on the data
/// directory, the audit log or the database's own files.
fn fs_code(e: &cw_fs::Error) -> &'static str {
    match e {
        cw_fs::Error::Io { .. } => "io_error",
        cw_fs::Error::Locked { .. } => "store_locked",
        cw_fs::Error::Symlink { .. } => "store_symlink",
        cw_fs::Error::ForeignOwner { .. } => "store_foreign_owner",
        cw_fs::Error::Permissions { .. } => "store_permissions",
        cw_fs::Error::NotRegular { .. } => "store_not_regular",
        cw_fs::Error::TooLarge { .. } => "store_too_large",
        cw_fs::Error::Reentrant { .. } => "store_lock_reentrant",
    }
}

/// The store's one translator into the workspace's error type, the kit's
/// `cw_base::Error` (STD-02 R12).
impl From<Error> for cw_base::Error {
    fn from(err: Error) -> Self {
        Self::new(err.kind(), err.code(), err)
    }
}

/// `Result` with this crate's [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
