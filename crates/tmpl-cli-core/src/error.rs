//! The crate's one error type.
//!
//! Callers branch on the variant (or on [`Error::code`]), never on the message
//! (STD-02 R10). `Display` carries only this layer's sentence; the underlying
//! cause stays on [`std::error::Error::source`], so a surface can print the
//! whole chain once without repeating it.

use crate::note::NoteId;
use std::io;
use std::path::PathBuf;

/// Everything a [`crate::Store`] operation can fail with.
///
/// `#[non_exhaustive]`: this is the library's public boundary (STD-02 R11), so
/// adding a variant is not a breaking change. Surfaces map it through
/// [`Error::code`] instead of matching variants.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// No note has this id. The message names the store file searched, so a
    /// mis-resolved data directory is visible (STD-02 R26).
    #[error("no note with id {id} in {}", path.display())]
    NotFound {
        /// The id that was looked up.
        id: NoteId,
        /// The store file that was searched.
        path: PathBuf,
    },
    /// A filesystem operation failed.
    #[error("cannot {action} {}", path.display())]
    Io {
        /// What was being attempted, as a verb phrase ("read", "create directory").
        action: &'static str,
        /// The path it was attempted on.
        path: PathBuf,
        /// The operating-system error.
        #[source]
        source: io::Error,
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
    /// Another writer held the store lock past the deadline (STD-03 R7).
    #[error(
        "{} is locked by {holder} (waited {waited_ms} ms); retry when it finishes",
        path.display()
    )]
    Locked {
        /// The lock file.
        path: PathBuf,
        /// Who holds it, as the holder recorded it (pid, label, time).
        holder: String,
        /// How long this call waited before giving up.
        waited_ms: u128,
    },
    /// A store file is a symbolic link (STD-05 R7, R9).
    #[error(
        "refusing {}: it is a symbolic link, and the store uses only regular files; replace the link with the file it points to",
        path.display()
    )]
    Symlink {
        /// The link.
        path: PathBuf,
    },
    /// Store state belongs to another user (STD-05 R9).
    #[error(
        "refusing {}: it is owned by uid {owner}, but this process runs as uid {uid}",
        path.display()
    )]
    ForeignOwner {
        /// The directory or file.
        path: PathBuf,
        /// Its owner.
        owner: u32,
        /// This process's effective uid.
        uid: u32,
    },
    /// Store state is writable by other users (STD-05 R9).
    #[error(
        "refusing {}: mode {mode:o} lets other users write it; fix with: chmod {fix} {}",
        path.display(),
        path.display()
    )]
    Permissions {
        /// The directory or file.
        path: PathBuf,
        /// Its permission bits.
        mode: u32,
        /// The owner-only mode to set: `700` for a directory, `600` for a file.
        fix: &'static str,
    },
}

impl Error {
    /// A stable `snake_case` code for this failure, for machine output
    /// (STD-01 R19). Adding a code is fine; changing one is a breaking change.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "note_not_found",
            Self::Io { .. } => "io_error",
            Self::Corrupt { .. } => "store_corrupt",
            Self::NewerFormat { .. } => "store_too_new",
            Self::Locked { .. } => "store_locked",
            Self::Symlink { .. } => "store_symlink",
            Self::ForeignOwner { .. } => "store_foreign_owner",
            Self::Permissions { .. } => "store_permissions",
        }
    }

    pub(crate) fn io(action: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            action,
            path: path.into(),
            source,
        }
    }
}

/// `Result` with this crate's [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
