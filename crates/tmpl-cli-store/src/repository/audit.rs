//! The audit repository: append-only audit events in `<root>/audit.jsonl`.

use crate::Store;
use crate::contracts::AuditEvent;
use crate::db::audit;
use crate::error::Result;

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
    pub fn append(&self, event: &AuditEvent) -> Result<()> {
        let path = self.store.audit_path();
        let line = audit::encode_line(event)?;
        if !cw_fs::inspect_dir(self.store.root())? {
            cw_fs::create_private_dir(self.store.root())?;
        }
        let _lock = self.store.audit_lock("audit")?;
        cw_fs::append(&path, &line)?;
        Ok(())
    }
}
