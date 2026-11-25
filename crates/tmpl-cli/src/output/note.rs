//! The `note` example's payloads and their rendering: one function per
//! shape and mode. Every rendering reads the same [`NotePayload`], so no
//! mode can show a value the others lack.

use super::color::role_for_priority;
use super::render::write_json;
use super::sink::{OutputMode, Sink};
use super::table::{self, Align, Cell, Column};
use serde::Serialize;
use std::io::{self, Write};
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
}

const LIST_COLUMNS: [Column; 5] = [
    Column {
        header: "ID",
        align: Align::Right,
        truncate: false,
    },
    Column {
        header: "PRIORITY",
        align: Align::Left,
        truncate: false,
    },
    Column {
        header: "TITLE",
        align: Align::Left,
        truncate: true,
    },
    Column {
        header: "TAGS",
        align: Align::Left,
        truncate: false,
    },
    Column {
        header: "CREATED",
        align: Align::Left,
        truncate: false,
    },
];

/// A list: table, tab-separated lines, or the JSON envelope carrying the
/// notes, `total` and `truncated` (STD-01 R34). `total` is how many matched.
pub(crate) fn list(
    out: &mut dyn Write,
    sink: &Sink,
    notes: &[NotePayload],
    total: usize,
) -> io::Result<()> {
    match sink.mode {
        OutputMode::Json => write_json(
            out,
            &ListPayload {
                notes,
                total,
                truncated: notes.len() < total,
            },
        ),
        // An empty list prints nothing, not a lone header (STD-01 R16).
        OutputMode::Table if notes.is_empty() => Ok(()),
        OutputMode::Table => {
            let rows: Vec<Vec<Cell>> = notes.iter().map(list_row).collect();
            out.write_all(table::render(&LIST_COLUMNS, &rows, sink.width, sink.color).as_bytes())
        }
        OutputMode::Plain => notes
            .iter()
            .try_for_each(|note| writeln!(out, "{}", plain_line(note))),
    }
}

/// One record: a field-per-line detail view, one tab-separated line, or a
/// JSON object. The detail view never truncates: it is where a truncated
/// table cell is read in full (STD-01 R15).
pub(crate) fn detail(out: &mut dyn Write, sink: &Sink, note: &NotePayload) -> io::Result<()> {
    match sink.mode {
        OutputMode::Json => write_json(out, note),
        OutputMode::Plain => writeln!(out, "{}", plain_line(note)),
        OutputMode::Table => {
            let priority = super::color::paint(
                note.priority.as_str(),
                role_for_priority(note.priority),
                sink.color,
            );
            let body = note
                .body
                .as_deref()
                .map(sanitize_for_display)
                .unwrap_or_else(|| String::from("-"))
                .replace('\n', "\n          ");
            writeln!(out, "id        {}", note.id)?;
            writeln!(out, "title     {}", note.title)?;
            writeln!(out, "priority  {priority}")?;
            writeln!(out, "tags      {}", tags_cell(&note.tags))?;
            writeln!(out, "created   {}", timestamp(note))?;
            writeln!(out, "body      {body}")
        }
    }
}

/// Neutralize control characters other than newline and tab so terminal
/// escape sequences and carriage returns cannot reach stdout raw.
fn sanitize_for_display(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' && chars.peek() == Some(&'\n') {
            continue;
        }
        if ch == '\n' || ch == '\t' || !ch.is_control() {
            out.push(ch);
        } else {
            for escaped in ch.escape_default() {
                out.push(escaped);
            }
        }
    }
    out
}

fn list_row(note: &NotePayload) -> Vec<Cell> {
    vec![
        Cell::plain(note.id.to_string()),
        Cell {
            text: note.priority.as_str().to_owned(),
            role: role_for_priority(note.priority),
        },
        Cell::plain(note.title.clone()),
        Cell::plain(tags_cell(&note.tags)),
        Cell::plain(timestamp(note)),
    ]
}

/// `id  priority  title  tags  created_at`, tab-separated. The body is left
/// out because it may span lines; `--json` and `note show` carry it.
pub(crate) fn plain_line(note: &NotePayload) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}",
        note.id,
        note.priority.as_str(),
        note.title,
        tags_cell(&note.tags),
        timestamp(note)
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
