//! Binary entry point for `tmpl-cli`: the one place that reads the
//! process. The environment is captured once, by the kit's `Env::capture()`,
//! and handed down with the arguments. `clippy.toml` refuses both reads
//! everywhere else.
#![expect(
    clippy::disallowed_methods,
    reason = "main is the one read of the process: Env::capture() and std::env::args_os()"
)]

use std::process::ExitCode;

fn main() -> ExitCode {
    let env = tmpl_cli_core::Env::capture();
    tmpl_cli::run(std::env::args_os().collect(), &env)
}
