//! Payload → bytes, one function per shape and mode. Every rendering reads
//! the same [`NotePayload`], so no mode can show a value the others lack.

use super::color::role_for_priority;
use super::payload::{ListPayload, NotePayload};
use super::sink::{OutputMode, Sink};
use super::table::{self, Align, Cell, Column};
use std::io::{self, Write};
use time::format_description::well_known::Rfc3339;

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
pub(crate) fn note_list(
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
            .try_for_each(|n| writeln!(out, "{}", plain_line(n))),
    }
}

/// One record: a field-per-line detail view, one tab-separated line, or a
/// JSON object. The detail view never truncates: it is where a truncated
/// table cell is read in full (STD-01 R15).
pub(crate) fn note_detail(out: &mut dyn Write, sink: &Sink, note: &NotePayload) -> io::Result<()> {
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
                .unwrap_or("-")
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

fn write_json<T: serde::Serialize + ?Sized>(out: &mut dyn Write, value: &T) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *out, value).map_err(io::Error::from)?;
    writeln!(out)
}
