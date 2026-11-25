//! `tmpl-cli`: the command-line surface over `tmpl-cli-core`.
//!
//! `run` is wiring only: parse, let the kit (cw-cli) make the one terminal
//! decision and open the output sink, start logging, start the core's
//! runtime (the data directory, the store, the clock, the audit log),
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

mod mcp;

mod output;

pub use cli::every_command;
pub use mcp::server as mcp_server;

use cli::Invocation;
use cw_cli::{Logging, Probe};
use std::ffi::OsString;
use std::process::ExitCode;
use tmpl_cli_core::{APP, Env, Origin, Surface};

/// The Markdown table of every environment variable the CLI reads: the
/// kit's first, then the CLI's own (`output::VARS`). `docs/usage.md` holds
/// it, and a golden test keeps the two equal.
#[must_use]
pub fn env_table() -> String {
    tmpl_cli_core::env::table(tmpl_cli_core::APP, output::VARS)
}

/// Run the CLI against `args`, the process arguments, and `env`, the
/// environment `main` captured once.
#[must_use]
pub fn run(args: &[OsString], env: &Env) -> ExitCode {
    // One probe of the streams; the kit decides colour, width and mode
    // from it and the captured environment.
    let probe = Probe::detect();
    let help_width = output::decide(env, probe, None).width();
    let cli = match cli::Cli::parse_args(args, help_width) {
        Ok(cli) => cli,
        Err(err) => {
            let requested = cli::raw_requested_format(args);
            return output::finish_parse_error(&err, env, probe, requested);
        }
    };
    let mut sink = output::sink(output::decide(env, probe, cli.global.requested_format()));
    if let Err(err) = cli.global.check() {
        return output::fail(&mut sink, &err);
    }
    Logging::from_env(env, APP).init();
    let runtime = match tmpl_cli_core::start(env, cli.global.root.as_deref()) {
        Ok(runtime) => runtime,
        Err(err) => return output::finish(&mut sink, Err(err)),
    };
    // The actor is the user the environment claims; the audit line records
    // it only when it reads as an identifier.
    let origin = Origin::new(Surface::CLI, env.user().map(str::to_owned));
    let command = match cli.command {
        Invocation::Run(command) => command,
        // The server names each call's origin itself, from the client.
        Invocation::Mcp(_args) => {
            return match mcp::serve(runtime) {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => output::fail(&mut sink, &err),
            };
        }
    };
    let (code, warning) = command::run(command, &runtime, &origin, |outcome| {
        output::finish(&mut sink, outcome)
    });
    if let Some(warning) = warning {
        output::audit_warning(&mut sink, &warning);
    }
    code
}

#[cfg(test)]
mod tests;
