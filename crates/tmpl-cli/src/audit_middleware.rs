//! Audit middleware: one audit-log line per mutating command.
//!
//! [`AuditGuard`] wraps a command's execution at the one chokepoint,
//! `commands::run` (STD-02 R24). It is created before dispatch, told the
//! outcome with [`AuditGuard::mark_result`], and writes the line when it is
//! dropped. It starts out recording a failure, so a command that returns early
//! or panics is logged as one; an outcome is never assumed to be success
//! (STD-02 R29).
//!
//! Which commands are audited is declared per command in
//! `commands::Command::audit`; read-only commands are not, because they must
//! not write (STD-01 R31). The line holds the command path, the record acted
//! on, the outcome and the stable error code: never argument values or error
//! text, which can carry note contents or paths (STD-05 R13).
//!
//! The audit log is a side channel, so it fails open (STD-02 R31): a line that
//! cannot be written never fails the command that already ran, and is reported
//! as a warning on stderr. `scripts/check-dependency-direction.sh` rejects
//! `crate::cli` and `crate::commands` imports here: dispatch depends on the
//! middleware, never the reverse.

use crate::error::CliError;
use crate::output::Output;
use std::time::Instant;
use time::OffsetDateTime;
use tmpl_cli_core::{AuditEvent, AuditStatus, Store};

/// How a command appears in the audit log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AuditMeta {
    /// The command path, such as `note add`.
    pub(crate) command: &'static str,
}

/// Records one command in the audit log when dropped.
#[must_use = "the audit line is written when the guard is dropped"]
#[derive(Debug)]
pub(crate) struct AuditGuard<'a> {
    store: &'a Store,
    meta: AuditMeta,
    at: OffsetDateTime,
    started: Instant,
    status: AuditStatus,
    target: Option<String>,
    error_code: Option<&'static str>,
}

impl<'a> AuditGuard<'a> {
    /// Start auditing a command that began at `at`. Until
    /// [`AuditGuard::mark_result`] is called, the command reads as failed.
    pub(crate) fn start(store: &'a Store, meta: AuditMeta, at: OffsetDateTime) -> Self {
        Self {
            store,
            meta,
            at,
            started: Instant::now(),
            status: AuditStatus::Failure,
            target: None,
            error_code: None,
        }
    }

    /// Record how the command ended: the note it acted on on success, the
    /// stable error code on failure (STD-01 R19).
    pub(crate) fn mark_result(&mut self, result: &Result<Output, CliError>) {
        match result {
            Ok(output) => {
                self.status = AuditStatus::Success;
                self.target = target_of(output);
                self.error_code = None;
            }
            Err(err) => {
                self.status = AuditStatus::Failure;
                self.target = None;
                self.error_code = Some(err.code());
            }
        }
    }

    /// The line this guard will write.
    pub(crate) fn event(&self) -> AuditEvent {
        AuditEvent {
            at: self.at,
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
        if let Err(err) = self.store.record_audit(&self.event()) {
            tracing::warn!(
                command = self.meta.command,
                path = %self.store.audit_path().display(),
                error = %err,
                "could not write the audit log"
            );
        }
    }
}

/// The id of the record a successful command acted on, if it names one.
/// Exhaustive, so a new payload decides what it contributes (STD-02 R27).
fn target_of(output: &Output) -> Option<String> {
    match output {
        Output::Note { note, .. } => Some(note.id.to_string()),
        Output::Notes { .. } => None,
    }
}
