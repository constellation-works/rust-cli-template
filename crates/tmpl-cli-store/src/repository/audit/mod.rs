//! The audit repository: append-only audit events in `<root>/audit.jsonl`.
//!
//! - `line`: the audit log's persisted line and its upgrades; bytes only,
//!   no I/O.
//!
//! The append reaches the file through the kit's `cw_fs` under its own lock
//! file, [`AUDIT_LOCK_FILE`].

pub(crate) mod line;

use crate::Store;
use crate::contract::AuditEvent;
use crate::error::Result;

/// The audit-log lock file's name under the data directory: an append holds
/// it, so concurrent audit lines never interleave.
pub(crate) const AUDIT_LOCK_FILE: &str = ".audit.lock";

/// The audit log of one store.
#[derive(Clone, Copy, Debug)]
pub struct Audit<'a> {
    store: &'a Store,
}

impl<'a> Audit<'a> {
    pub(crate) fn new(store: &'a Store) -> Self {
        Self { store }
    }

    /// Append one event, creating the data directory and the log owner-only
    /// if needed. The append holds its own `.audit.lock` so concurrent audit
    /// lines never interleave and a store-lock timeout can still record its
    /// failure line immediately. An unsafe data directory is refused as for
    /// any write.
    ///
    /// # Errors
    ///
    /// The store's typed `Error`, translated into `cw_base::Error` here at
    /// the crate's boundary.
    pub fn append(&self, event: &AuditEvent) -> std::result::Result<(), cw_base::Error> {
        Ok(self.append_line(event)?)
    }

    fn append_line(&self, event: &AuditEvent) -> Result<()> {
        let path = self.store.audit_path();
        let line = line::encode_line(event)?;
        if !cw_fs::inspect_dir(self.store.root())? {
            cw_fs::create_private_dir(self.store.root())?;
        }
        let _lock = self.store.audit_lock("audit")?;
        cw_fs::append(&path, &line)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
