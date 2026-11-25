//! The data directory's files: their names and their persisted formats.
//!
//! - `format`: the persisted notes document, its format versions and upgrades;
//! - `audit`: the audit log's persisted line and its upgrades.
//!
//! Both convert bytes and do no I/O. No code in this crate names `std::fs`:
//! the [`Store`] facade and the repositories reach a file through the
//! kit's `cw_fs`
//! (`scripts/check-dependency-direction.sh`).
//!
//! [`Store`]: crate::Store

pub(crate) mod audit;
pub(crate) mod format;

/// The store file's name under the data directory.
pub const STORE_FILE: &str = "notes.json";

/// The most bytes a store file may hold before a read refuses it
/// (`store_too_large`): a file that grew without bound must not exhaust
/// memory. Far above any store this tool writes.
pub(crate) const STORE_CAP: cw_fs::Cap = cw_fs::Cap::bytes(64 * 1024 * 1024);

/// The store write lock file's name under the data directory.
pub(crate) const LOCK_FILE: &str = ".lock";

/// The audit-log lock file's name under the data directory, separate from
/// [`LOCK_FILE`] so a lock timeout on a note write does not block its audit line.
pub(crate) const AUDIT_LOCK_FILE: &str = ".audit.lock";

#[cfg(test)]
pub(crate) mod tests;
