//! The data directory's files and the mechanisms that touch them.
//!
//! - `format`: the persisted notes document, its format versions and upgrades;
//! - `audit`: the audit log's persisted line and its upgrades;
//! - `fsio`: durable writes, appends, locks and private-state checks,
//!   mechanism only, with no knowledge of notes.
//!
//! Only this module touches the filesystem; repositories and the [`Store`]
//! facade reach a file through it. `format` and `audit` do no I/O of their
//! own, and `fsio` imports nothing else from the crate but its error type
//! (`scripts/check-dependency-direction.sh`).
//!
//! [`Store`]: crate::Store

pub(crate) mod audit;
pub(crate) mod format;
pub(crate) mod fsio;

/// The store file's name under the data directory.
pub const STORE_FILE: &str = "notes.json";

/// The store write lock file's name under the data directory.
pub(crate) const LOCK_FILE: &str = ".lock";

/// The audit-log lock file's name under the data directory, separate from
/// [`LOCK_FILE`] so a lock timeout on a note write does not block its audit line.
pub(crate) const AUDIT_LOCK_FILE: &str = ".audit.lock";

#[cfg(test)]
pub(crate) mod tests;
