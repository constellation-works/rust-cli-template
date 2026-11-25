//! `tmpl-cli`: keep short notes in a local store.
//!
//! `run` is wiring only: parse, resolve the output sink once, hand the
//! command to its app in `tmpl-cli-core` (which resolves the data directory
//! and audits the command), render. See ARCHITECTURE.md for the layers and
//! docs/design/tmpl-cli/ for the design.

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

use std::process::ExitCode;
use tmpl_cli_core::Env;

/// Run the CLI against the process arguments and `env`, the environment
/// `main` captured once.
#[must_use]
pub fn run(env: Env) -> ExitCode {
    let args: Vec<_> = std::env::args_os().collect();
    let cli = match cli::Cli::parse_args(&args) {
        Ok(cli) => cli,
        Err(err) => return output::finish_parse_error(&err, &args),
    };
    let sink = output::Sink::from_process(cli.global.requested_format());
    output::init_logging();
    let env = env.with_root_flag(cli.global.root);
    command::run_with(cli.command, &env, |outcome| output::finish(&sink, outcome))
}

#[cfg(test)]
mod tests;
