//! The root of the command tree: the program, its global flags and the
//! parse entry point. Each noun's subcommands, flags and help live beside its
//! handlers in `commands/` (STD-01 R25: declared once). `--help` is rendered
//! from these doc comments, so they are user-facing text: placeholders like
//! `<id>`, never real ids or tracker numbers (STD-01 R23).

use crate::commands::Command;
use crate::output::FormatArg;
use clap::error::ErrorKind;
use clap::{Args, CommandFactory, Parser};
use std::ffi::OsString;
use std::path::PathBuf;

/// Help heading for the flags every command accepts.
const GLOBAL: &str = "Global options";

const ROOT_EXAMPLES: &str = "\
Examples:
  tmpl-cli note add \"Call the plumber\" --tag home --priority high
  tmpl-cli note list --tag home
  tmpl-cli note show <id> --json
  tmpl-cli note list | cut -f1,3

Exit status: 0 on success, 1 when a command fails, 2 on a usage error.";

/// Keep short notes in a local store.
#[derive(Debug, Parser)]
#[command(
    name = "tmpl-cli",
    version,
    arg_required_else_help = true,
    propagate_version = true,
    after_help = ROOT_EXAMPLES
)]
pub(crate) struct Cli {
    #[command(flatten)]
    pub(crate) global: GlobalArgs,

    #[command(subcommand)]
    pub(crate) command: Command,
}

/// Flags accepted before or after any subcommand (STD-01 R4).
#[derive(Debug, Args)]
pub(crate) struct GlobalArgs {
    /// Data directory [default: $TMPL_CLI_ROOT, else ~/.tmpl-cli]
    #[arg(long, global = true, value_name = "DIR", help_heading = GLOBAL)]
    pub(crate) root: Option<PathBuf>,

    /// Output format; auto is a table on a terminal, tab-separated lines otherwise, and json is one JSON document [default: $TMPL_CLI_FORMAT, else auto]
    #[arg(long, global = true, value_enum, value_name = "FORMAT", help_heading = GLOBAL)]
    pub(crate) format: Option<FormatArg>,

    /// Shorthand for --format json
    #[arg(long, global = true, help_heading = GLOBAL)]
    pub(crate) json: bool,
}

impl Cli {
    /// Parse a command line, including the checks clap cannot express.
    ///
    /// `conflicts_with` does not see a global flag given at a different
    /// subcommand level (`--json note list --format table`), so the
    /// `--json`/`--format` conflict is checked here and reported as the same
    /// usage error (exit 2) clap would have produced.
    pub(crate) fn parse_args(args: &[OsString]) -> Result<Self, clap::Error> {
        let cli = Self::try_parse_from(args)?;
        if cli.global.json && cli.global.format.is_some_and(|f| f != FormatArg::Json) {
            return Err(Self::command().error(
                ErrorKind::ArgumentConflict,
                "--json cannot be used with a --format other than json",
            ));
        }
        Ok(cli)
    }
}

impl GlobalArgs {
    /// The output format the flags ask for, or `None` to defer to the
    /// environment and then `auto`.
    pub(crate) fn requested_format(&self) -> Option<FormatArg> {
        if self.json {
            Some(FormatArg::Json)
        } else {
            self.format
        }
    }
}
