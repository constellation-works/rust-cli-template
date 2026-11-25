//! Binary entry point for `tmpl-cli`: the one place that reads the
//! process. The environment is captured once, by the kit's `Env::capture()`,
//! and handed down with the arguments; nothing else in the template names
//! `std::env` (`scripts/check-dependency-direction.sh`).

use std::process::ExitCode;

fn main() -> ExitCode {
    let env = tmpl_cli_core::Env::capture();
    tmpl_cli::run(std::env::args_os().collect(), &env)
}
