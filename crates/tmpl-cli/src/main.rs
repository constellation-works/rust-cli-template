//! Binary entry point for `tmpl-cli`: the one place that reads the
//! arguments. The environment is captured once, by the core's
//! `capture_env()` (no read of the environment lives in this crate,
//! STD-02 R7), and handed down with the arguments. `clippy.toml` refuses
//! both reads everywhere else.
#![expect(
    clippy::disallowed_methods,
    reason = "main is the one read of the arguments: std::env::args_os()"
)]

use std::process::ExitCode;

fn main() -> ExitCode {
    let env = tmpl_cli_core::capture_env();
    let args: Vec<_> = std::env::args_os().collect();
    tmpl_cli::run(&args, &env)
}
