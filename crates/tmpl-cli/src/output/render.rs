//! Payload → bytes for the payloads every kind of this CLI has, and what
//! every rendering shares.

use super::color::Role;
use super::payload::StatusPayload;
use super::sink::{OutputMode, Sink};
use super::table::{self, Align, Cell, Column};
use std::io::{self, Write};

const STATUS_COLUMNS: [Column; 2] = [
    Column {
        header: "ROOT",
        align: Align::Left,
        truncate: false,
    },
    Column {
        header: "FORMAT",
        align: Align::Right,
        truncate: false,
    },
];

/// The store's status: a one-row table, one tab-separated line
/// (`root  format`), or a JSON object. A store with no database yet has no
/// format: `-` in the human modes, `null` in JSON (STD-01 R11, R14).
pub(crate) fn status(out: &mut dyn Write, sink: &Sink, status: &StatusPayload) -> io::Result<()> {
    let format = status
        .format
        .map_or_else(|| String::from("-"), |format| format.to_string());
    match sink.mode {
        OutputMode::Json => write_json(out, status),
        OutputMode::Plain => writeln!(out, "{}\t{format}", status.root),
        OutputMode::Table => {
            let role = if status.format.is_some() {
                Role::Neutral
            } else {
                Role::Muted
            };
            let row = vec![
                Cell::plain(status.root.clone()),
                Cell { text: format, role },
            ];
            out.write_all(table::render(&STATUS_COLUMNS, &[row], sink.width, sink.color).as_bytes())
        }
    }
}

/// One JSON document, pretty-printed, ending in a newline.
pub(super) fn write_json<T: serde::Serialize + ?Sized>(
    out: &mut dyn Write,
    value: &T,
) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *out, value).map_err(io::Error::from)?;
    writeln!(out)
}
