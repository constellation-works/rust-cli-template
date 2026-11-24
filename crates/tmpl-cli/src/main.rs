//! Binary entry point for `tmpl-cli`.

use std::process::ExitCode;

fn main() -> ExitCode {
    tmpl_cli::run(tmpl_cli_core::Env::from_process())
}
