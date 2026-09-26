//! `tmpl-cli`: keep short notes in a local store.
//!
//! `main` is wiring only: parse, resolve the output sink once, resolve the
//! context, run the command, render. See ARCHITECTURE.md for the layers and
//! docs/design/tmpl-cli/ for the design.

// Nothing in this crate uses `print!`: the output module writes to locked
// handles so a closed pipe is an error value, not a panic (STD-01 R13).
#![deny(clippy::print_stderr, clippy::print_stdout)]
// Unit tests use unwrap/expect for fixture setup; production call sites remain linted.
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

mod app;
mod cli;
mod command;
mod error;
mod output;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().collect();
    let cli = match cli::Cli::parse_args(&args) {
        Ok(cli) => cli,
        Err(err) => return output::finish_parse_error(&err, &args),
    };
    let sink = output::Sink::from_process(cli.global.requested_format());
    output::init_logging();
    let outcome = app::Context::resolve(cli.global.root.as_deref(), &app::AppEnv::from_process())
        .and_then(|ctx| command::run(cli.command, &ctx));
    output::finish(&sink, outcome)
}

#[cfg(test)]
mod tests;
