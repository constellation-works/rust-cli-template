//! Error and usage-error rendering through the kit's sink.

use super::terminal::{decide, sink};
use crate::error::{hint, output_error, usage_error};
use cw_cli::{Exit, Format, Mode, Probe, Sink};
use std::io::Write;
use std::process::ExitCode;
use tmpl_cli_core::{Env, Error};

/// Report a failure that is not a command's outcome (a usage error the
/// surface raises itself, the MCP server's own) and return its exit code.
pub(crate) fn fail(sink: &mut Sink, err: &Error) -> ExitCode {
    report(sink, err)
}

/// Report `err` on stderr, with its fix where this surface knows one: one
/// `error:` line and a `hint:` line, or one JSON object in JSON mode
/// (STD-01 R19, R21).
pub(super) fn report<O: Write, E: Write>(sink: &mut Sink<O, E>, err: &Error) -> ExitCode {
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
