//! The store's one error type, and the bad-record report a read carries.
//!
//! Callers branch on the variant (or on [`Error::code`]), never on the message
//! (STD-02 R10). `Display` carries only this layer's sentence; the underlying
//! cause stays on [`std::error::Error::source`], so a surface can print the
//! whole chain once without repeating it. The one translator into the
//! workspace's error type is the `From` impl at the end of this file
//! (STD-02 R12).

use crate::contracts::{NoteError, NoteId};
use cw_base::Kind;
use std::fmt;
use std::path::PathBuf;

/// Where a bad record sits in the store file: its note id when readable,
/// otherwise its 1-based line number in the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordLocator {
    /// The record had a readable note id.
    Id(NoteId),
    /// The record's id itself was missing or unreadable; identified by its
    /// 1-based line number in the store file.
    Line(usize),
}

impl fmt::Display for RecordLocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Id(id) => write!(f, "note {id}"),
            Self::Line(line) => write!(f, "line {line}"),
        }
    }
}

/// Why a stored note record failed validation.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BadRecordReason {
    /// The note entry was not a JSON object.
    #[error("expected a JSON object")]
    NotAnObject,
    /// A required field was absent from the stored note.
    #[error("missing required field '{0}'")]
    MissingField(&'static str),
    /// A field had the wrong JSON type or an unparseable value.
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
    /// The store file holding the record.
    pub path: PathBuf,
    /// The record's id, or its 1-based line number when the id is unreadable.
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
    /// A file operation on the data directory failed, was refused, or timed
    /// out on the lock. The kit's error carries a kind and no code: the
    /// store names the code ([`Error::code`]).
    #[error(transparent)]
    Fs(#[from] cw_fs::Error),
    /// Serializing a store document or audit event failed before any file was touched.
    #[error("cannot encode {what}")]
    Encode {
        /// What was being serialized.
        what: &'static str,
        /// What the serializer rejected.
        #[source]
        source: serde_json::Error,
    },
    /// The store file exists but is not a store this build can parse.
    #[error("{} is not a valid store file", path.display())]
    Corrupt {
        /// The store file.
        path: PathBuf,
        /// What the parser rejected.
        #[source]
        source: serde_json::Error,
    },
    /// One or more note records in the store failed validation.
    #[error(transparent)]
    BadRecord(#[from] BadRecord),
    /// Two notes in the store share the same id.
    #[error("{} contains duplicate note id {id}", path.display())]
    DuplicateId {
        /// The store file.
        path: PathBuf,
        /// The repeated id.
        id: NoteId,
    },
    /// The store was written by a newer build (STD-03 R10: refuse, never
    /// rewrite or reinterpret newer state).
    #[error(
        "{} uses store format {found}, but this build reads format {supported} or older; upgrade this tool",
        path.display()
    )]
    NewerFormat {
        /// The store file.
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
            Self::Fs(cw_fs::Error::Io { .. }) => "io_error",
            Self::Fs(cw_fs::Error::Locked { .. }) => "store_locked",
            Self::Fs(cw_fs::Error::Symlink { .. }) => "store_symlink",
            Self::Fs(cw_fs::Error::ForeignOwner { .. }) => "store_foreign_owner",
            Self::Fs(cw_fs::Error::Permissions { .. }) => "store_permissions",
            Self::Fs(cw_fs::Error::NotRegular { .. }) => "store_not_regular",
            Self::Fs(cw_fs::Error::TooLarge { .. }) => "store_too_large",
            Self::Fs(cw_fs::Error::Reentrant { .. }) => "store_lock_reentrant",
            Self::Encode { .. } => "encode_error",
            Self::Corrupt { .. } => "store_corrupt",
            Self::BadRecord(_) => "store_bad_record",
            Self::DuplicateId { .. } => "store_duplicate_id",
            Self::NewerFormat { .. } => "store_too_new",
        }
    }

    /// How this failure is classified, for the workspace's error type.
    /// Exhaustive, so a new variant decides (STD-02 R27).
    #[must_use]
    pub fn kind(&self) -> Kind {
        match self {
            Self::Fs(e) => e.kind(),
            Self::Encode { .. } => Kind::Internal,
            Self::Corrupt { .. } | Self::BadRecord(_) | Self::DuplicateId { .. } => Kind::Corrupt,
            Self::NewerFormat { .. } => Kind::Incompatible,
        }
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
