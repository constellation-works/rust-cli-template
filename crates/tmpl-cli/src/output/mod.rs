//! The output layer: the only code that writes to stdout or stderr.
//!
//! stdout carries the payload and nothing else; notices, errors and logs go
//! to stderr in every mode (STD-01 R12). Writes go through `writeln!` on a
//! locked handle rather than `println!`, so a closed pipe comes back as an
//! `io::Error` that [`finish`] turns into a silent exit 0 (STD-01 R13)
//! instead of a panic.

mod color;

mod env;

// example:begin
mod note;

// example:end
mod payload;

mod render;

mod sink;

mod table;

pub(crate) use payload::{Output, Payload, StatusPayload};
// example:begin
pub(crate) use note::NotePayload;
// example:end
pub(crate) use env::VARS;
pub(crate) use sink::{FormatArg, Sink, help_width};

use crate::error::{exit_code, hint, output_error};
use serde::Serialize;
use std::ffi::OsString;
use std::fmt::Display;
use std::io::{self, Write};
use std::process::ExitCode;
use tmpl_cli_core::{APP, Env, Error};

/// Send `tracing` output to stderr as plain text, filtered by `TMPL_CLI_LOG`
/// from the captured environment (`warn` when unset or empty).
///
/// Fail open (STD-02 R31): logging is a side channel, so a filter that does
/// not parse never fails the command. It is reported once and replaced by
/// the default, not silently ignored.
pub(crate) fn init_logging(env: &Env) {
    use tracing_subscriber::EnvFilter;
    let default = || EnvFilter::new("warn");
    let app = env.app(APP);
    let log_var = tmpl_cli_core::env::LOG.name_for(APP);
    let filter = match app.log().map(|spec| spec.to_str().ok_or(spec)) {
        Some(Ok(spec)) => EnvFilter::try_new(spec).unwrap_or_else(|err| {
            let _ = writeln!(
                io::stderr().lock(),
                "warning: ignoring {log_var}={spec:?} ({err}); logging at warn"
            );
            default()
        }),
        None => default(),
        Some(Err(_not_utf8)) => {
            let _ = writeln!(
                io::stderr().lock(),
                "warning: ignoring {log_var}: not valid UTF-8; logging at warn"
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
pub(crate) fn finish(sink: &Sink, outcome: Result<Output, Error>) -> ExitCode {
    let err = match outcome {
        Ok(output) => match emit(sink, &output) {
            Ok(()) => return ExitCode::SUCCESS,
            Err(write_err) if write_err.kind() == io::ErrorKind::BrokenPipe => {
                return ExitCode::SUCCESS;
            }
            Err(write_err) => output_error(write_err),
        },
        Err(err) => err,
    };
    report_error(sink.mode == sink::OutputMode::Json, &err);
    ExitCode::from(exit_code(&err))
}

/// Handle an argument-parsing outcome that is not a parsed command line:
/// `--help` and `--version` (stdout, exit 0) or a usage error (stderr, exit 2).
pub(crate) fn finish_parse_error(err: &clap::Error, args: &[OsString], env: &Env) -> ExitCode {
    let code = u8::try_from(err.exit_code()).unwrap_or(2);
    if !err.use_stderr() {
        let written = io::stdout()
            .lock()
            .write_all(err.render().to_string().as_bytes());
        return match written {
            Err(write_err) if write_err.kind() != io::ErrorKind::BrokenPipe => ExitCode::from(1),
            _ => ExitCode::from(code),
        };
    }
    let env_format = sink::env_format(env);
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

/// Print the audit log's warning after the command's own output: the line
/// for a command that already ran could not be written. Fail open
/// (STD-02 R31): it never changes the exit code.
pub(crate) fn audit_warning(warning: &impl Display) {
    let _ = writeln!(io::stderr().lock(), "warning: {warning}");
}

fn emit(sink: &Sink, output: &Output) -> io::Result<()> {
    let Output {
        payload,
        notice,
        warnings,
    } = output;
    if !warnings.is_empty() || notice.is_some() {
        // Fail open (STD-02 R31): warnings and notices that cannot reach stderr
        // never fail a command whose payload is fine.
        let mut err = io::stderr().lock();
        for warning in warnings {
            let _ = writeln!(err, "warning: {warning}");
        }
        if let Some(notice) = notice {
            let _ = writeln!(err, "{notice}");
        }
    }
    let mut out = io::stdout().lock();
    match payload {
        Payload::Status(status) => render::status(&mut out, sink, status)?,
        // example:begin
        Payload::Notes { notes, total } => note::list(&mut out, sink, notes, *total)?,
        Payload::Note(note) => note::detail(&mut out, sink, note)?,
        // example:end
    }
    out.flush()
}

/// Report an error on stderr: one JSON object in JSON mode, else one
/// `error: <message>` line with the cause chain folded in (STD-01 R19).
fn report_error(json: bool, err: &Error) {
    let mut message = err.to_string();
    let mut source = std::error::Error::source(err);
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    if let Some(hint) = hint(err) {
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
