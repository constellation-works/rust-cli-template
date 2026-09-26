//! The machine-output contract (STD-01 R6, R10, R11).
//!
//! Every rendering — table, tab-separated, JSON — is derived from these
//! structs, so the human and machine views cannot disagree about what exists.
//! The JSON field names and types are a public contract: renaming, removing
//! or retyping one is a breaking change, pinned by the output goldens. This
//! shape is deliberately separate from the store's persisted document.

use serde::Serialize;
use time::OffsetDateTime;
use tmpl_cli_core::{Note, Priority};

/// One note as every output mode sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct NotePayload {
    /// The note's id.
    pub(crate) id: u64,
    /// One line of text.
    pub(crate) title: String,
    /// Longer text; `null` when absent, never omitted (R11).
    pub(crate) body: Option<String>,
    /// Labels, sorted; `[]` when none.
    pub(crate) tags: Vec<String>,
    /// Canonical lowercase token: `low`, `normal` or `high` (R11).
    pub(crate) priority: Priority,
    /// RFC 3339 with an explicit offset (R11).
    #[serde(with = "time::serde::rfc3339")]
    pub(crate) created_at: OffsetDateTime,
}

impl From<&Note> for NotePayload {
    fn from(note: &Note) -> Self {
        Self {
            id: note.id.get(),
            title: note.title.as_str().to_owned(),
            body: note.body.clone(),
            tags: note.tags.iter().map(|t| t.as_str().to_owned()).collect(),
            priority: note.priority,
            created_at: note.created_at,
        }
    }
}

/// The `--json` form of a list: always this envelope, whether or not a limit
/// cut it, so its shape never depends on the flags (STD-01 R34, R10).
#[derive(Debug, Serialize)]
pub(crate) struct ListPayload<'a> {
    /// The notes returned, in order.
    pub(crate) notes: &'a [NotePayload],
    /// How many notes matched before `--limit` was applied.
    pub(crate) total: usize,
    /// Whether `--limit` left matching notes out.
    pub(crate) truncated: bool,
}

/// What a command produced, before rendering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Output {
    /// A list result.
    Notes {
        /// The notes returned, in order.
        notes: Vec<NotePayload>,
        /// How many matched before the limit; more than `notes.len()` when
        /// the list was cut.
        total: usize,
        /// One stderr line, printed in every mode when present (STD-01 R12):
        /// what was searched when nothing matched (R16), or that the list
        /// was cut (R34).
        notice: Option<String>,
    },
    /// A single record (`show`, `add`).
    Note {
        /// The record.
        note: NotePayload,
        /// One stderr line when present: for a write, the record and the
        /// file it went to (STD-01 R30).
        notice: Option<String>,
    },
}
