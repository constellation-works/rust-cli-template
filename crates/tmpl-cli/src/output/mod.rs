//! The output layer: the only code that writes to stdout or stderr.
//!
//! stdout carries the payload and nothing else; notices, errors and logs go
//! to stderr in every mode (STD-01 R12). Writes go through `writeln!` on a
//! locked handle rather than `println!`, so a closed pipe comes back as an
//! `io::Error` that [`finish`] turns into a silent exit 0 (STD-01 R13)
//! instead of a panic.

mod color;
mod payload;
mod render;
mod sink;
mod table;

pub(crate) use payload::{NotePayload, Output};
pub(crate) use sink::{FORMAT_ENV, FormatArg, Sink};

use crate::error::CliError;
use serde::Serialize;
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

/// Environment variable holding the log filter (`warn` when unset).
const LOG_ENV: &str = "TMPL_CLI_LOG";

/// Send `tracing` output to stderr as plain text, filtered by `TMPL_CLI_LOG`.
///
/// Fail open (STD-02 R31): logging is a side channel, so a filter that does
/// not parse never fails the command. It is reported once and replaced by
/// the default, not silently ignored.
pub(crate) fn init_logging() {
    use tracing_subscriber::EnvFilter;
    let default = || EnvFilter::new("warn");
    let filter = match std::env::var(LOG_ENV) {
        Ok(spec) => EnvFilter::try_new(&spec).unwrap_or_else(|e| {
            let _ = writeln!(
                io::stderr().lock(),
                "warning: ignoring {LOG_ENV}={spec:?} ({e}); logging at warn"
            );
            default()
        }),
        Err(std::env::VarError::NotPresent) => default(),
        Err(std::env::VarError::NotUnicode(_)) => {
            let _ = writeln!(
                io::stderr().lock(),
                "warning: ignoring {LOG_ENV}: not valid UTF-8; logging at warn"
            );
            default()
        }
    };
    // Ignore the error: it only means a subscriber is already installed.
    let _ = tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .try_init();
}

/// Render a command's outcome and return the process exit code.
pub(crate) fn finish(sink: &Sink, outcome: Result<Output, CliError>) -> ExitCode {
    let result = outcome.and_then(|output| emit(sink, &output).map_err(CliError::Output));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(CliError::Output(e)) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(err) => {
            report_error(sink.mode == sink::OutputMode::Json, &err);
            ExitCode::from(err.exit_code())
        }
    }
}

/// Handle an argument-parsing outcome that is not a parsed command line:
/// `--help` and `--version` (stdout, exit 0) or a usage error (stderr, exit 2).
pub(crate) fn finish_parse_error(err: &clap::Error, args: &[OsString]) -> ExitCode {
    let code = u8::try_from(err.exit_code()).unwrap_or(2);
    if !err.use_stderr() {
        let written = io::stdout()
            .lock()
            .write_all(err.render().to_string().as_bytes());
        return match written {
            Err(e) if e.kind() != io::ErrorKind::BrokenPipe => ExitCode::from(1),
            _ => ExitCode::from(code),
        };
    }
    let env_format = std::env::var(FORMAT_ENV).ok();
    let rendered = err.render().to_string();
    if sink::raw_args_request_json(args, env_format.as_deref()) {
        let first = rendered.lines().next().unwrap_or_default();
        let message = first.strip_prefix("error: ").unwrap_or(first);
        write_json_error(message, "usage_error");
    } else {
        let _ = io::stderr().lock().write_all(rendered.as_bytes());
    }
    ExitCode::from(code)
}

fn emit(sink: &Sink, output: &Output) -> io::Result<()> {
    let (Output::Notes { notice, .. } | Output::Note { notice, .. }) = output;
    if let Some(notice) = notice {
        // Fail open (STD-02 R31): a notice that cannot reach stderr never
        // fails a command whose payload is fine.
        let _ = writeln!(io::stderr().lock(), "{notice}");
    }
    let mut out = io::stdout().lock();
    match output {
        Output::Notes { notes, total, .. } => render::note_list(&mut out, sink, notes, *total)?,
        Output::Note { note, .. } => render::note_detail(&mut out, sink, note)?,
    }
    out.flush()
}

/// Report an error on stderr: one JSON object in JSON mode, else one
/// `error: <message>` line with the cause chain folded in (STD-01 R19).
fn report_error(json: bool, err: &CliError) {
    let mut message = err.to_string();
    let mut source = std::error::Error::source(err);
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    if let Some(hint) = err.hint() {
        message.push_str("; ");
        message.push_str(hint);
    }
    if json {
        write_json_error(&message, err.code());
    } else {
        let _ = writeln!(io::stderr().lock(), "error: {message}");
    }
}

#[derive(Serialize)]
struct ErrorPayload<'a> {
    error: &'a str,
    code: &'a str,
}

fn write_json_error(message: &str, code: &str) {
    let payload = ErrorPayload {
        error: message,
        code,
    };
    let mut err = io::stderr().lock();
    if serde_json::to_writer(&mut err, &payload).is_ok() {
        let _ = writeln!(err);
    }
}

#[cfg(test)]
mod tests;
