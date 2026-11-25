//! The root of the command tree: the program, its global flags and the
//! parse entry point. Each noun's subcommands, flags and help live beside its
//! handlers in `command/` (STD-01 R25: declared once). `--help` is rendered
//! from these doc comments, so they are user-facing text: placeholders like
//! `<id>`, never real ids or tracker numbers (STD-01 R23).

use crate::command::Command;
use crate::output::FormatArg;
use clap::error::ErrorKind;
use clap::{Args, CommandFactory, FromArgMatches, Parser};
use std::ffi::OsString;
use std::path::PathBuf;

/// Help heading for the flags every command accepts.
const GLOBAL: &str = "Global options";

const ROOT_EXAMPLES: &str = concat!(
    "Examples:\n",
    "  tmpl-cli status\n",
    // example:begin
    "  tmpl-cli note add \"Call the plumber\" --tag home --priority high\n",
    "  tmpl-cli note list --tag home\n",
    "  tmpl-cli note show <id> --json\n",
    "  tmpl-cli note list | cut -f1,3\n",
    // example:end
    "\n",
    "Exit status: 0 on success, 1 when a command fails, 2 on a usage error.",
);

/// A command-line tool with a private local store.
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

    /// Output format; auto is a table on a terminal, tab-separated lines otherwise,
    /// and json is one JSON document [default: $TMPL_CLI_FORMAT, else auto]
    #[arg(long, global = true, value_enum, value_name = "FORMAT", help_heading = GLOBAL)]
    pub(crate) format: Option<FormatArg>,

    /// Shorthand for --format json [default: false]
    #[arg(long, global = true, help_heading = GLOBAL)]
    pub(crate) json: bool,
}

impl Cli {
    /// Parse a command line, including the checks clap cannot express.
    ///
    /// `conflicts_with` does not see a global flag given at a different
    /// subcommand level (`--json status --format table`), so the
    /// `--json`/`--format` conflict is checked here and reported as the same
    /// usage error (exit 2) clap would have produced. `help_width` is the
    /// width root help wraps to, `None` off a terminal.
    pub(crate) fn parse_args(
        args: &[OsString],
        help_width: Option<usize>,
    ) -> Result<Self, clap::Error> {
        let matches =
            crate::help::with_groups(Self::command(), help_width).try_get_matches_from(args)?;
        let cli = Self::from_arg_matches(&matches)?;
        if cli.global.json
            && cli
                .global
                .format
                .is_some_and(|format| format != FormatArg::Json)
        {
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

/// Every command path in the CLI tree, starting with the root (`"tmpl-cli"`),
/// paired with its built [`clap::Command`]. Used by help and golden tests so a
/// newly added command cannot be forgotten (STD-01 R22, R24).
#[must_use]
pub fn every_command() -> Vec<(String, clap::Command)> {
    fn collect(
        cmd: &clap::Command,
        path: &mut Vec<String>,
        out: &mut Vec<(String, clap::Command)>,
    ) {
        out.push((path.join(" "), cmd.clone()));
        for sub in cmd.get_subcommands() {
            if sub.get_name() == "help" {
                continue;
            }
            path.push(sub.get_name().to_owned());
            collect(sub, path, out);
            path.pop();
        }
    }
    let mut root = crate::help::with_groups(Cli::command(), None);
    root.build();
    let mut out = Vec::new();
    collect(&root, &mut vec![String::from("tmpl-cli")], &mut out);
    out
}
