//! The `note` example's payloads and their human views: the table a list
//! is, the one-row table and the detail text a single note is. The kit's
//! sink picks the view for its mode and writes the payload itself as JSON,
//! so no mode can show a value the others lack.

use cw_cli::{Cell, Column, Role, Table};
use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tmpl_cli_core::contract::{Note, Priority};

/// One note as every output mode sees it (STD-01 R6, R10, R11). The JSON
/// field names and types are a public contract, pinned by the output
/// goldens.
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
            tags: note
                .tags
                .iter()
                .map(|tag| tag.as_str().to_owned())
                .collect(),
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
    /// Each record skipped because it could not be read, as stderr warns of
    /// it too; `[]` when none, never omitted (STD-02 R32, STD-01 R11).
    pub(crate) warnings: &'a [SkippedRecord],
}

/// A store record a list skipped: which one, and why it could not be read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct SkippedRecord {
    /// The record, as the store names it: `note <id>`, or `row <id>` when
    /// the row's id is not a note id.
    pub(crate) record: String,
    /// Why it was skipped.
    pub(crate) reason: String,
}

/// The one mapping from domain values to colour roles (STD-01 R18).
pub(super) fn role_for_priority(priority: Priority) -> Role {
    match priority {
        Priority::High => Role::Warn,
        Priority::Low => Role::Muted,
        Priority::Normal => Role::Neutral,
    }
}

/// A list's human view: a table, or one tab-separated line per note with
/// no header (STD-01 R9, R14). An empty list prints nothing (STD-01 R16).
pub(super) fn list_table(notes: &[NotePayload]) -> Table {
    let mut table = Table::new([
        Column::new("ID").right(),
        Column::new("PRIORITY"),
        Column::new("TITLE").truncatable(),
        Column::new("TAGS"),
        Column::new("CREATED"),
    ]);
    for note in notes {
        table.push([
            Cell::new(note.id.to_string()),
            Cell::new(note.priority.as_str()).role(role_for_priority(note.priority)),
            Cell::new(note.title.clone()),
            Cell::new(tags_cell(&note.tags)),
            Cell::new(timestamp(note)),
        ]);
    }
    table
}

/// One note as a field-per-line detail view, for a reader at a terminal or
/// asking for a table. It never truncates: it is where a cut table cell is
/// read in full (STD-01 R15). A body's line breaks are kept, indented under
/// its column, and a CR before a line feed is dropped; the sink escapes
/// every other control character.
pub(super) fn detail_text(note: &NotePayload) -> String {
    let body = note.body.as_deref().map_or_else(
        || String::from("-"),
        |body| body.replace("\r\n", "\n").replace('\n', "\n          "),
    );
    format!(
        "id        {}\ntitle     {}\npriority  {}\ntags      {}\ncreated   {}\nbody      {body}\n",
        note.id,
        note.title,
        note.priority.as_str(),
        tags_cell(&note.tags),
        timestamp(note),
    )
}

fn tags_cell(tags: &[String]) -> String {
    if tags.is_empty() {
        String::from("-")
    } else {
        tags.join(",")
    }
}

fn timestamp(note: &NotePayload) -> String {
    note.created_at
        .format(&Rfc3339)
        .unwrap_or_else(|_| String::from("-"))
}
