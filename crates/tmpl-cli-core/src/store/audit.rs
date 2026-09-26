//! The audit log's record shape: one JSON line per mutating command.
//!
//! `<root>/audit.jsonl` answers "what changed this store, when, and did it
//! work". The CLI's audit middleware builds each [`AuditEvent`] and
//! [`crate::Store::record_audit`] appends it. Read-only commands are never
//! audited: they must not write (STD-01 R31).
//!
//! An event holds stable identifiers only: the command path, the record it
//! acted on, the outcome and a stable error code. Never argument values or
//! error text, which can carry note contents or paths (STD-05 R13). Each line
//! carries its own `format` so a reader can refuse a newer one (STD-03 R10);
//! a crash can leave a torn last line, which a reader skips.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// The audit log's name under the data directory.
pub const AUDIT_FILE: &str = "audit.jsonl";

/// The newest audit line format. Bump it, and keep reading the old one, when
/// the line's shape changes (STD-02 R16).
pub(super) const AUDIT_FORMAT: u32 = 1;

/// How an audited command ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditStatus {
    /// The command completed.
    Success,
    /// The command returned an error, or never reported a result (a panic or
    /// an early return), which reads as failure, never as success (STD-02 R29).
    Failure,
}

/// One audited command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    /// When the command started.
    #[serde(with = "time::serde::rfc3339")]
    pub at: OffsetDateTime,
    /// The command path, such as `note add`.
    pub command: String,
    /// How it ended.
    pub status: AuditStatus,
    /// The id of the record it acted on, when it names one.
    #[serde(default)]
    pub target: Option<String>,
    /// The stable error code on failure (STD-01 R19); never the message.
    #[serde(default)]
    pub error_code: Option<String>,
    /// Wall time from start to result, in milliseconds.
    pub duration_ms: u64,
}

/// A persisted line: the event plus its format.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct AuditLine {
    pub(super) format: u32,
    #[serde(flatten)]
    pub(super) event: AuditEvent,
}

/// The event as one newline-terminated JSON line at the current format.
pub(super) fn encode_line(event: &AuditEvent) -> serde_json::Result<Vec<u8>> {
    let mut line = serde_json::to_vec(&AuditLine {
        format: AUDIT_FORMAT,
        event: event.clone(),
    })?;
    line.push(b'\n');
    Ok(line)
}
