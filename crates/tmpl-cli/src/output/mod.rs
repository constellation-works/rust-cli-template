//! The output layer: what this program shows, written through the kit.
//!
//! The mechanism is cw-cli's: one terminal decision ([`Terminal`]), the sink
//! that is the only path to stdout and stderr ([`Sink`]), error rendering
//! with the exit code from the error's kind ([`Sink::report`]), and the
//! table layout. stdout carries the payload and nothing else; notices,
//! warnings and errors go to stderr in every mode (STD-01 R12), and a closed
//! reader ends the writes, not the command (STD-01 R13). What stays here is
//! the program's own: its payloads, each command's human view (a table, or
//! the detail text of one record), and the colour role of each value.

mod env;

// example:begin
mod note;

// example:end
mod payload;

mod status;

pub(crate) use payload::{Output, Payload, StatusPayload};
// example:begin
pub(crate) use note::{NotePayload, SkippedRecord};
// example:end
pub(crate) use env::VARS;

use crate::error::{hint, output_error, usage_error};
use cw_cli::{Exit, Format, Mode, Probe, Sink, Terminal};
use std::io::Write;
use std::process::ExitCode;
use tmpl_cli_core::{APP, Env, Error};

/// The terminal decision for this invocation, made from the environment
/// `main` captured, one `probe` of the streams and the format the command
/// line asked for (`None` defers to `TMPL_CLI_FORMAT`, then `auto`).
pub(crate) fn decide(env: &Env, probe: Probe, requested: Option<Format>) -> Terminal {
    Terminal::decide(env, APP, probe, requested)
}

/// A sink on the process's stdout and stderr, rendering as `terminal`
/// decided.
pub(crate) fn sink(terminal: Terminal) -> Sink {
    Sink::stdio(terminal)
}

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

/// Report a failure that is not a command's outcome (a usage error the
/// surface raises itself, the MCP server's own) and return its exit code.
pub(crate) fn fail(sink: &mut Sink, err: &Error) -> ExitCode {
    report(sink, err)
}

/// Report `err` on stderr, with its fix where this surface knows one: one
/// `error:` line and a `hint:` line, or one JSON object in JSON mode
/// (STD-01 R19, R21).
fn report<O: Write, E: Write>(sink: &mut Sink<O, E>, err: &Error) -> ExitCode {
    sink.report(err, hint(err)).into()
}

/// Handle an argument-parsing outcome that is not a parsed command line:
/// `--help` and `--version` (stdout, exit 0) or a usage error (stderr, exit
/// 2). `requested` is the format the raw arguments asked for, since they
/// did not parse.
pub(crate) fn finish_parse_error(
    err: &clap::Error,
    env: &Env,
    probe: Probe,
    requested: Option<Format>,
) -> ExitCode {
    let rendered = err.render().to_string();
    if !err.use_stderr() {
        // Help and version are text in every mode.
        let mut sink = sink(decide(env, probe, Some(Format::Table)));
        let written = sink
            .emit_text(rendered.as_str(), ToOwned::to_owned)
            .and_then(|()| sink.finish());
        return match written {
            Ok(()) => ExitCode::SUCCESS,
            Err(write_err) => report(&mut sink, &output_error(write_err)),
        };
    }
    let mut sink = sink(decide(env, probe, requested));
    let (first, tail) = rendered.split_once('\n').unwrap_or((&rendered, ""));
    let Some(message) = first.strip_prefix("error: ") else {
        // A missing subcommand: clap's help, on stderr, is the message.
        if sink.terminal().mode() == Mode::Json {
            return report(&mut sink, &usage_error(first));
        }
        sink.notice(rendered.trim_end_matches('\n'));
        return Exit::of(tmpl_cli_core::Kind::Usage).into();
    };
    if sink.terminal().mode() == Mode::Json {
        // One JSON object: the lines that name the argument join the
        // message, and clap's usage and tip are its hint.
        let (named, rest) = json_usage(message, tail);
        let err = usage_error(&named);
        let hint = (!rest.is_empty()).then_some(rest.as_str());
        return sink.report(&err, hint).into();
    }
    let exit = report(&mut sink, &usage_error(message));
    // As text, clap's usage and tip follow the error line unchanged.
    let tail = tail.trim_end_matches('\n');
    if !tail.is_empty() {
        sink.notice(tail);
    }
    exit
}

/// A clap usage error as one JSON object's message and hint. The lines
/// right after clap's error line, before the first blank one, name the
/// argument (`  <ID>`) and join the message; the rest (a tip, the usage
/// line, the pointer to `--help`) is the hint, one line each.
fn json_usage(first_line: &str, tail: &str) -> (String, String) {
    let mut lines = tail.lines();
    let named: Vec<&str> = lines
        .by_ref()
        .take_while(|line| !line.trim().is_empty())
        .map(str::trim)
        .collect();
    let message = if named.is_empty() {
        first_line.trim().to_owned()
    } else {
        format!("{} {}", first_line.trim(), named.join(", "))
    };
    let rest = lines
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    (message, rest)
}

/// Print the audit log's warning after the command's own output: the line
/// for a command that already ran could not be written. Fail open
/// (STD-02 R31): it never changes the exit code.
pub(crate) fn audit_warning(sink: &mut Sink, warning: &impl std::fmt::Display) {
    sink.warn(&warning.to_string());
}

/// Warnings and the notice to stderr, then the payload to stdout in the
/// sink's mode, then the flush.
fn emit<O: Write, E: Write>(sink: &mut Sink<O, E>, output: &Output) -> Result<(), Error> {
    let Output {
        payload,
        notice,
        warnings,
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
        Payload::Notes { notes, .. } => sink.emit(&json, |_| note::list_table(notes)),
        // A piped `show` is the list's one tab-separated line; a terminal
        // or `--format table` gets the detail view.
        Payload::Note(note) if sink.terminal().mode() == Mode::Plain => {
            sink.emit(&json, |_| note::list_table(std::slice::from_ref(note)))
        }
        Payload::Note(note) => sink.emit_text(&json, |_| note::detail_text(note)),
        // example:end
    };
    emitted.and_then(|()| sink.finish()).map_err(output_error)
}

#[cfg(test)]
mod tests;
