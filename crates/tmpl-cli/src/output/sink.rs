//! The output sink: one answer per invocation to "who is reading stdout?".
//!
//! This is the only module that looks at terminal state or at the output
//! environment variables (STD-01 R8, R17); `scripts/check-terminal-guard.sh`
//! fails CI if anything else does. It reads them from the environment
//! `main` captured once ([`Env`]), never from the process. `run` resolves a
//! [`Sink`] once and every renderer reads its answers from it.

use super::color::Color;
use super::env::{CLICOLOR_FORCE, COLUMNS};
use clap::ValueEnum;
use std::ffi::{OsStr, OsString};
use std::io::IsTerminal;
use tmpl_cli_core::{APP, Env};

/// A captured value as text: `None` when unset, empty or not UTF-8.
fn text(value: Option<&OsStr>) -> Option<String> {
    value.and_then(OsStr::to_str).map(str::to_owned)
}

/// `TMPL_CLI_FORMAT`, as text.
pub(crate) fn env_format(env: &Env) -> Option<String> {
    text(env.app(APP).format())
}

/// The `--format` values.
///
/// The variants carry `//` comments, not `///`: clap would render doc
/// comments as per-value help, which switches every `--help` page to the
/// sprawling long layout. The flag's own help explains the values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum FormatArg {
    // A table on a terminal, tab-separated lines otherwise.
    Auto,
    // An aligned table with a header row.
    Table,
    // One JSON document: an array for a list, an object for one record.
    Json,
}

/// How this invocation renders its payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutputMode {
    /// Borderless aligned table with a header (STD-01 R14).
    Table,
    /// One tab-separated line per record, no header (STD-01 R9).
    Plain,
    /// One JSON document (STD-01 R7).
    Json,
}

/// The output-relevant environment, captured once. Tests build this by hand
/// instead of mutating the process environment.
#[derive(Clone, Debug, Default)]
pub(crate) struct SinkEnv {
    /// `TMPL_CLI_FORMAT`.
    pub(crate) format: Option<String>,
    /// `NO_COLOR`: any non-empty value disables color.
    pub(crate) no_color: Option<String>,
    /// `CLICOLOR_FORCE`: any non-empty value enables color on a terminal.
    pub(crate) clicolor_force: Option<String>,
    /// `TERM`: `dumb` disables color.
    pub(crate) term: Option<String>,
    /// `COLUMNS`: the width to fit tables to on a terminal.
    pub(crate) columns: Option<String>,
}

impl SinkEnv {
    /// Take the variables from the environment `main` captured.
    pub(crate) fn from_env(env: &Env) -> Self {
        let app = env.app(APP);
        Self {
            format: env_format(env),
            no_color: env.no_color().then(|| String::from("1")),
            clicolor_force: text(app.get(&CLICOLOR_FORCE)),
            term: text(env.term()),
            columns: text(app.get(&COLUMNS)),
        }
    }
}

/// Everything a renderer may know about its destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Sink {
    /// The resolved output mode.
    pub(crate) mode: OutputMode,
    /// Whether ANSI color may be written to stdout.
    pub(super) color: Color,
    /// The width a table must fit, or `None` for "never truncate".
    pub(crate) width: Option<usize>,
}

impl Sink {
    /// Resolve the sink from the real stdout and the captured environment.
    /// Called once.
    pub(crate) fn from_env(env: &Env, requested: Option<FormatArg>) -> Self {
        Self::resolve(
            std::io::stdout().is_terminal(),
            &SinkEnv::from_env(env),
            requested,
        )
    }

    /// Resolve from explicit inputs; the testable core of [`Sink::from_env`].
    pub(crate) fn resolve(is_tty: bool, env: &SinkEnv, requested: Option<FormatArg>) -> Self {
        Self {
            mode: resolve_mode(is_tty, env.format.as_deref(), requested),
            color: resolve_color(is_tty, env),
            width: resolve_width(is_tty, env.columns.as_deref()),
        }
    }
}

/// Precedence: explicit flag > `TMPL_CLI_FORMAT` > `auto`. An unrecognized
/// environment value falls back to `auto` instead of failing (STD-01 R8).
pub(crate) fn resolve_mode(
    is_tty: bool,
    env_format: Option<&str>,
    requested: Option<FormatArg>,
) -> OutputMode {
    let from_env = env_format.and_then(|value| FormatArg::from_str(value.trim(), true).ok());
    match requested.or(from_env).unwrap_or(FormatArg::Auto) {
        FormatArg::Json => OutputMode::Json,
        FormatArg::Table => OutputMode::Table,
        FormatArg::Auto if is_tty => OutputMode::Table,
        FormatArg::Auto => OutputMode::Plain,
    }
}

/// Color is off off a terminal, under a non-empty `NO_COLOR`, and for
/// `TERM=dumb`; a non-empty `CLICOLOR_FORCE` overrides `TERM=dumb` but never
/// `NO_COLOR` or a pipe (STD-01 R17).
pub(super) fn resolve_color(is_tty: bool, env: &SinkEnv) -> Color {
    let set = |value: &Option<String>| value.as_deref().is_some_and(|text| !text.is_empty());
    if !is_tty || set(&env.no_color) {
        return Color::Plain;
    }
    if set(&env.clicolor_force) || env.term.as_deref() != Some("dumb") {
        return Color::Ansi;
    }
    Color::Plain
}

/// Width for root help wrapping. Piped help keeps one line per command.
pub(crate) fn help_width(env: &Env) -> Option<usize> {
    let columns = text(env.app(APP).get(&COLUMNS));
    resolve_width(std::io::stdout().is_terminal(), columns.as_deref())
}

/// Only a terminal has a width; piped output is never truncated (STD-01 R9).
fn resolve_width(is_tty: bool, columns: Option<&str>) -> Option<usize> {
    if !is_tty {
        return None;
    }
    columns
        .and_then(|text| text.trim().parse::<usize>().ok())
        .filter(|&width| width > 0)
}

/// Whether a command line that failed to parse asked for JSON, so the usage
/// error can be reported as JSON too (STD-01 R19). Parsing failed, so this
/// reads the raw arguments: `--json`, `--format json` or `--format=json`,
/// else the environment variable.
pub(crate) fn raw_args_request_json(args: &[OsString], env_format: Option<&str>) -> bool {
    let mut explicit = None;
    let mut iter = args.iter().skip(1).map(|os_arg| os_arg.to_string_lossy());
    while let Some(arg) = iter.next() {
        match arg.as_ref() {
            "--" => break,
            "--json" => explicit = Some(true),
            "--format" => explicit = iter.next().map(|value| value == "json"),
            other => {
                if let Some(value) = other.strip_prefix("--format=") {
                    explicit = Some(value == "json");
                }
            }
        }
    }
    explicit.unwrap_or_else(|| {
        env_format.is_some_and(|value| value.trim().eq_ignore_ascii_case("json"))
    })
}
