//! The environment variables the CLI reads beyond the ones the kit
//! declares, for the usage table. [`VARS`] lists them beside the kit's own
//! list (`tmpl_cli_core::env::KIT_VARS`) in `crate::env_table`, which
//! `docs/usage.md` holds.
//!
//! The two here are read by the kit itself (`cw_cli::Terminal::decide`),
//! which `cw_app` does not declare yet; they are declared here only so the
//! table keeps listing them, and go once the kit declares them. A variable
//! the CLI reads itself is declared here and read through its declaration.

use tmpl_cli_core::EnvVar;

/// Forces color on a terminal even under `TERM=dumb`.
pub(crate) const CLICOLOR_FORCE: EnvVar = EnvVar::global(
    "CLICOLOR_FORCE",
    "Any non-empty value turns color on at a terminal, even for `TERM=dumb`.",
);

/// The width tables and root help fit on a terminal.
pub(crate) const COLUMNS: EnvVar =
    EnvVar::global("COLUMNS", "The terminal width to fit tables to.");

/// Every variable the CLI reads beyond the kit's, in the order the usage
/// table lists them after the kit's.
pub(crate) const VARS: &[EnvVar] = &[CLICOLOR_FORCE, COLUMNS];
