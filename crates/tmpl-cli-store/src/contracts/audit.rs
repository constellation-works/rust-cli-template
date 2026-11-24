//! The audit log's record shape.
//!
//! The core's audit guard builds one [`AuditEvent`] per mutating command;
//! read-only commands are never audited, because they must not write
//! (STD-01 R31). An event holds stable identifiers only: the actor, the
//! command path, the record it acted on, the outcome and a stable error code.
//! Never argument values or error text, which can carry note contents or
//! paths (STD-05 R13). How a line is persisted is `db/audit.rs`.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

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
    /// The OS user name that invoked the command, or `None` when unset.
    #[serde(default)]
    pub actor: Option<String>,
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
