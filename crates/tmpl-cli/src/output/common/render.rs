//! Writing a command's output and warnings through the kit's sink.

use super::error::report;
use super::payload::{Output, Payload};
use crate::error;
// example:begin
use crate::output::note::{self, ListedNotePayload, NoteSummaryPayload};
// example:end
use crate::output::status;
// example:begin
use cw_cli::Mode;
// example:end
use cw_cli::Sink;
use std::fmt::Display;
use std::io::Write;
use std::process::ExitCode;
use tmpl_cli_core::Error;

/// Render a command's outcome and return the process exit code: 0, or the
/// code the failure's kind calls for, from the kit (STD-01 R20).
pub(crate) fn finish<O: Write, E: Write>(
    sink: &mut Sink<O, E>,
    outcome: Result<Output, Error>,
) -> ExitCode {
    match outcome.and_then(|output| emit(sink, &output)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => report(sink, &err),
    }
}

/// Warnings and the notice to stderr, then the payload to stdout in the
/// sink's mode, then the flush.
fn emit<O: Write, E: Write>(sink: &mut Sink<O, E>, output: &Output) -> Result<(), Error> {
    let Output {
        payload,
        notice,
        warnings,
        // example:begin
        skipped: _,
        // example:end
    } = output;

    for warning in warnings {
        sink.warn(warning);
    }

    if let Some(notice) = notice {
        sink.notice(notice);
    }

    let json = payload.json();
    let emitted = match payload {
        Payload::Status(status) => sink.emit(&json, |_| status::to_table(status)),
        // example:begin
        Payload::Notes {
            notes,
            include_body,
            ..
        } => sink.emit(&json, |_| note::list_table(notes, *include_body)),
        // A piped `show` is the list's one tab-separated line; a terminal
        // or `--format table` gets the detail view.
        Payload::Note(note) if sink.terminal().mode() == Mode::Plain => sink.emit(&json, |_| {
            note::list_table(
                &[ListedNotePayload::Summary(NoteSummaryPayload::from(note))],
                false,
            )
        }),
        Payload::Note(note) => sink.emit_text(&json, |_| note::detail_text(note)),
        // example:end
    };

    emitted
        .and_then(|()| sink.finish())
        .map_err(error::output_error)
}

/// Print the audit log's warning after the command's own output: the line
/// for a command that already ran could not be written. Fail open
/// (STD-02 R31): it never changes the exit code.
pub(crate) fn audit_warning(sink: &mut Sink, warning: &impl Display) {
    sink.warn(&warning.to_string());
}
