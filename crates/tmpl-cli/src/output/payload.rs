//! The machine-output contract (STD-01 R6, R10, R11).
//!
//! Every rendering — table, tab-separated, JSON — is derived from these
//! structs, so the human and machine views cannot disagree about what exists.
//! The JSON field names and types are a public contract: renaming, removing
//! or retyping one is a breaking change, pinned by the output goldens. This
//! shape is deliberately separate from the store's persisted document.

use serde::Serialize;
// example:begin

use super::note::NotePayload;
// example:end

/// The store's status as every output mode sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct StatusPayload {
    /// The data directory.
    pub(crate) root: String,
    /// The database's store format; `null` when there is no database yet,
    /// never omitted (R11).
    pub(crate) format: Option<u32>,
}

/// What a command produced, before rendering: the payload for stdout and
/// the lines for stderr.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Output {
    /// What goes to stdout.
    pub(crate) payload: Payload,
    /// One stderr line, printed in every mode when present (STD-01 R12):
    /// for a write, the record and the file it went to (R30); for a list,
    /// what was searched when nothing matched (R16) or that it was cut
    /// (R34).
    pub(crate) notice: Option<String>,
    /// One stderr warning per bad record isolated while reading the store.
    pub(crate) warnings: Vec<String>,
}

impl Output {
    /// `body` with no notice and no warning.
    pub(crate) fn new(payload: Payload) -> Self {
        Self {
            payload,
            notice: None,
            warnings: Vec::new(),
        }
    }
}

/// The payload a command produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Payload {
    /// The store's status.
    Status(StatusPayload),
    // example:begin
    /// A list of notes.
    Notes {
        /// The notes returned, in order.
        notes: Vec<NotePayload>,
        /// How many matched before the limit; more than `notes.len()` when
        /// the list was cut.
        total: usize,
    },
    /// A single note (`show`, `add`).
    Note(NotePayload),
    // example:end
}
