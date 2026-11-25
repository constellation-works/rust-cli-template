//! The audit log's file: its name, its lock file and its persisted line.
//!
//! - `audit`: the audit log's persisted line and its upgrades; bytes only,
//!   no I/O.
//!
//! The notes are not here: they live in the database the driver sets up
//! (`crate::driver::sqlite`). No code in this crate names `std::fs`: the
//! audit repository reaches its file through the kit's `cw_fs`
//! (`scripts/check-dependency-direction.sh`).

pub(crate) mod audit;

/// The audit-log lock file's name under the data directory: an append holds
/// it, so concurrent audit lines never interleave.
pub(crate) const AUDIT_LOCK_FILE: &str = ".audit.lock";

#[cfg(test)]
pub(crate) mod tests;
