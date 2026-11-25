//! `tmpl-cli`: the command-line surface over `tmpl-cli-core`.
//!
//! `run` is wiring only: parse, resolve the output sink once, start the
//! core's runtime (the data directory, the store, the clock, the audit log),
//! hand the command to its app through the kit's chokepoint, render the
//! result, then settle its audit record and print a failed audit write as a
//! warning. See ARCHITECTURE.md for the layers and docs/design/tmpl-cli/ for
//! the design.

// Nothing in this crate uses `print!`: the output module writes to locked
// handles so a closed pipe is an error value, not a panic (STD-01 R13).
#![deny(clippy::print_stderr, clippy::print_stdout)]
// Unit tests use unwrap/expect for fixture setup; production call sites remain linted.
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "unit tests assert fixture setup"
    )
)]

mod cli;

mod command;

mod error;

mod help;

mod output;

pub use cli::every_command;

use std::ffi::OsString;
use std::process::ExitCode;
use tmpl_cli_core::{Env, Origin, Surface};

/// Run the CLI against `args`, the process arguments, and `env`, the
/// environment `main` captured once.
#[must_use]
pub fn run(args: Vec<OsString>, env: &Env) -> ExitCode {
    let cli = match cli::Cli::parse_args(&args, output::help_width(env)) {
        Ok(cli) => cli,
        Err(err) => return output::finish_parse_error(&err, &args, env),
    };
    let sink = output::Sink::from_env(env, cli.global.requested_format());
    output::init_logging(env);
    let runtime = match tmpl_cli_core::start(env, cli.global.root.as_deref()) {
        Ok(runtime) => runtime,
        Err(err) => return output::finish(&sink, Err(err)),
    };
    // The actor is the user the environment claims; the audit line records
    // it only when it reads as an identifier.
    let origin = Origin::new(Surface::CLI, env.user().map(str::to_owned));
    let (code, warning) = command::run(cli.command, &runtime, &origin, |outcome| {
        output::finish(&sink, outcome)
    });
    if let Some(warning) = warning {
        output::audit_warning(&warning);
    }
    code
}

#[cfg(test)]
mod tests;
