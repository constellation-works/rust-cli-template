//! The data directory's files: their names and their persisted formats.
//!
//! - `format`: the persisted notes document, its format versions and upgrades;
//! - `audit`: the audit log's persisted line and its upgrades.
//!
//! Both convert bytes and do no I/O. No code in this crate names `std::fs`:
//! the [`Store`] facade and the repositories reach a file through the
//! foundation's `tmpl_cli_common::fs`
//! (`scripts/check-dependency-direction.sh`).
//!
//! [`Store`]: crate::Store

pub(crate) mod audit;
pub(crate) mod format;

/// The store file's name under the data directory.
pub const STORE_FILE: &str = "notes.json";

/// The store write lock file's name under the data directory.
pub(crate) const LOCK_FILE: &str = ".lock";

/// The audit-log lock file's name under the data directory, separate from
/// [`LOCK_FILE`] so a lock timeout on a note write does not block its audit line.
pub(crate) const AUDIT_LOCK_FILE: &str = ".audit.lock";

#[cfg(test)]
pub(crate) mod tests;
