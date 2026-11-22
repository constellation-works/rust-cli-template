//! The audit log's record shape: one JSON line per mutating command.
//!
//! `<root>/audit.jsonl` answers "who changed this store, what changed it,
//! when, and did it work". The CLI's audit middleware builds each
//! [`AuditEvent`] and [`crate::Store::record_audit`] appends it. Read-only
//! commands are never audited: they must not write (STD-01 R31).
//!
//! An event holds stable identifiers only: the actor, the command path, the
//! record it acted on, the outcome and a stable error code. Never argument
//! values or error text, which can carry note contents or paths (STD-05 R13).
//! Each line carries its own `format` so a reader can upgrade older lines and
//! refuse newer ones (STD-03 R10, R23); a crash can leave a torn last line,
//! which a reader skips.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use std::path::Path;
use time::OffsetDateTime;

/// The audit log's name under the data directory.
pub(super) const AUDIT_FILE: &str = "audit.jsonl";

/// The newest audit line format. Bump it and append a step to [`UPGRADES`]
/// whenever the line's shape changes (STD-02 R16, STD-03 R23).
pub(super) const AUDIT_FORMAT: u32 = 2;

/// One upgrade step: a format `n` audit line in, the format `n + 1` line out.
pub(super) type AuditUpgrade = fn(serde_json::Value) -> serde_json::Result<serde_json::Value>;

/// The audit-line upgrade registry, append-only (STD-03 R23). `UPGRADES[i]`
/// turns format `i + 1` into format `i + 2`, so `AUDIT_FORMAT` is always
/// `UPGRADES.len() + 1` (a unit test checks). A shipped step is never edited,
/// reordered or removed.
pub(super) const UPGRADES: &[AuditUpgrade] = &[upgrade_v1_to_v2];

/// Format 1 -> 2: introduce optional `actor` (defaulting to `null` for v1 lines).
fn upgrade_v1_to_v2(mut line: serde_json::Value) -> serde_json::Result<serde_json::Value> {
    if let Some(obj) = line.as_object_mut() {
        obj.entry("actor").or_insert(serde_json::Value::Null);
        obj.insert(String::from("format"), serde_json::Value::from(2u32));
    }
    Ok(line)
}

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

/// A persisted line: the event plus its format.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct AuditLine {
    pub(super) format: u32,
    #[serde(flatten)]
    pub(super) event: AuditEvent,
}

/// Just the format number, read before the full line so a newer line is
/// refused by version, not by parse error.
#[derive(Deserialize)]
struct FormatProbe {
    format: NonZeroU32,
}

/// The event as one newline-terminated JSON line at the current format.
pub(super) fn encode_line(event: &AuditEvent) -> Result<Vec<u8>> {
    let mut line = serde_json::to_vec(&AuditLine {
        format: AUDIT_FORMAT,
        event: event.clone(),
    })
    .map_err(|source| Error::Encode {
        what: "audit event",
        source,
    })?;
    line.push(b'\n');
    Ok(line)
}

/// Parse one persisted audit line, upgrading older formats in memory and
/// refusing newer ones (STD-03 R10, R23).
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "template mechanism for an audit reader")
)]
pub(super) fn decode_line(path: &Path, bytes: &[u8]) -> Result<AuditLine> {
    let corrupt = |source| Error::Corrupt {
        path: path.to_path_buf(),
        source,
    };
    let mut value: serde_json::Value = serde_json::from_slice(bytes).map_err(corrupt)?;
    let from = FormatProbe::deserialize(&value)
        .map_err(corrupt)?
        .format
        .get();
    if from > AUDIT_FORMAT {
        return Err(Error::NewerFormat {
            path: path.to_path_buf(),
            found: from,
            supported: AUDIT_FORMAT,
        });
    }
    for (step, to) in UPGRADES.iter().zip(2..) {
        if to > from {
            value = step(value).map_err(corrupt)?;
        }
    }
    serde_json::from_value(value).map_err(corrupt)
}
