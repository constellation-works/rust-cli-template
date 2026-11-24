//! What a file operation of this module fails with.
//!
//! `Display` carries only this layer's sentence; the operating-system cause
//! stays on [`std::error::Error::source`]. The calling unit wraps this in its
//! own typed error and decides the stable code; [`Error::kind`] is the
//! classification, decided once here.

use crate::error::Kind;
use std::io;
use std::path::PathBuf;

/// Why a file operation failed or was refused.
///
/// Not `#[non_exhaustive]`: a unit that wraps it gives every variant a
/// stable code, so adding a variant is meant to break its match.
#[derive(Debug, thiserror::Error)]
pub enum Error {
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
    /// Another writer held the lock past the deadline (STD-03 R7).
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
    /// A file is a symbolic link (STD-05 R7, R9).
    #[error(
        "refusing {}: it is a symbolic link, and the store uses only regular files; replace the link with the file it points to",
        path.display()
    )]
    Symlink {
        /// The link.
        path: PathBuf,
    },
    /// The file or directory belongs to another user (STD-05 R9).
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
    /// The file or directory is writable by other users (STD-05 R9).
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
    /// How this failure is classified. Exhaustive, so a new variant decides
    /// (STD-02 R27).
    #[must_use]
    pub fn kind(&self) -> Kind {
        match self {
            Self::Io { .. } => Kind::Io,
            Self::Locked { .. } => Kind::Busy,
            Self::Symlink { .. } | Self::ForeignOwner { .. } | Self::Permissions { .. } => {
                Kind::Refused
            }
        }
    }

    pub(super) fn io(action: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            action,
            path: path.into(),
            source,
        }
    }
}

/// `Result` with this module's [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
