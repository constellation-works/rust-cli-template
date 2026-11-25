//! The audit guard: one audit-log line per mutating command.
//!
//! [`AuditGuard`] wraps a command's execution at the one chokepoint, the
//! runtime's `exec` (STD-02 R24). It is created before the flow runs, told the
//! outcome with [`AuditGuard::mark_result`], and writes the line when it is
//! dropped, after the caller has rendered the outcome. It starts out
//! recording a failure, so a command that returns early or panics is logged
//! as one; an outcome is never assumed to be success (STD-02 R29).
//!
//! Which commands are audited is declared per command by each app's
//! `Command::audit`; read-only commands are not, because they must not write
//! (STD-01 R31). The line holds the actor, the command path, the
//! record acted on, the outcome and the stable error code: never argument
//! values or error text, which can carry record contents or paths (STD-05 R13).
//!
//! The audit log is a side channel, so it fails open (STD-02 R31): a line that
//! cannot be written never fails the command that already ran, and is reported
//! as a warning through `tracing`. `scripts/check-dependency-direction.sh`
//! rejects `crate::app` imports in the runtime: apps depend on it, never the
//! reverse.

use std::time::Instant;
use time::OffsetDateTime;
use tmpl_cli_store::Store;
use tmpl_cli_store::contract::{AuditEvent, AuditStatus};

/// How a command appears in the audit log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AuditMeta {
    /// The command path, such as `record add`.
    pub(crate) command: &'static str,
}

/// Records one command in the audit log when dropped.
#[must_use = "the audit line is written when the guard is dropped"]
#[derive(Debug)]
pub(crate) struct AuditGuard<'a> {
    store: &'a Store,
    meta: AuditMeta,
    at: OffsetDateTime,
    actor: Option<String>,
    started: Instant,
    status: AuditStatus,
    target: Option<String>,
    error_code: Option<&'static str>,
}

impl<'a> AuditGuard<'a> {
    /// Start auditing a command that began at `at`. Until
    /// [`AuditGuard::mark_result`] is called, the command reads as failed.
    pub(crate) fn start(
        store: &'a Store,
        meta: AuditMeta,
        at: OffsetDateTime,
        actor: Option<String>,
    ) -> Self {
        Self {
            store,
            meta,
            at,
            actor,
            started: Instant::now(),
            status: AuditStatus::Failure,
            target: None,
            error_code: None,
        }
    }

    /// Record how the command ended: on success the id of the record it
    /// acted on, if it names one; on failure the stable error code
    /// (STD-01 R19).
    pub(crate) fn mark_result(&mut self, result: Result<Option<String>, &'static str>) {
        match result {
            Ok(target) => {
                self.status = AuditStatus::Success;
                self.target = target;
                self.error_code = None;
            }
            Err(code) => {
                self.status = AuditStatus::Failure;
                self.target = None;
                self.error_code = Some(code);
            }
        }
    }

    /// The line this guard will write.
    pub(crate) fn event(&self) -> AuditEvent {
        AuditEvent {
            at: self.at,
            actor: self.actor.clone(),
            command: self.meta.command.to_owned(),
            status: self.status,
            target: self.target.clone(),
            error_code: self.error_code.map(str::to_owned),
            duration_ms: u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX),
        }
    }
}

impl Drop for AuditGuard<'_> {
    fn drop(&mut self) {
        // Fail open, and never panic in a destructor (STD-03 R4).
        if let Err(err) = self.store.audit().append(&self.event()) {
            tracing::warn!(
                command = self.meta.command,
                path = %self.store.audit_path().display(),
                error = %err,
                "could not write the audit log"
            );
        }
    }
}
