//! Declares the root command and global flags; imports command and error.
//!
//! The root of the command tree: the program, its global flags and the
//! parse entry point. Each noun's subcommands, flags and help live beside its
//! handlers in `command/` (STD-01 R25: declared once). `--help` is rendered
//! from these doc comments, so they are user-facing text: placeholders like
//! `<id>`, never real ids or tracker numbers (STD-01 R23).

use crate::command::Command;
use crate::error::usage_error;
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use cw_cli::Format;
use cw_cli::help::{Group, grouped};
use std::ffi::OsString;
use std::path::PathBuf;
use tmpl_cli_core::env::{FORMAT, ROOT};
use tmpl_cli_core::{APP, Error};

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

/// The root help's command groups, in order; the kit renders each command
/// under its heading with its one-line summary.
const GROUPS: &[Group<'_>] = &[
    // example:begin
    Group {
        heading: "Notes",
        commands: &["note"],
    },
    // example:end
    Group {
        heading: "Store",
        commands: &["status"],
    },
    Group {
        heading: "Agents",
        commands: &["mcp"],
    },
    Group {
        heading: "Help",
        commands: &["help"],
    },
];

const MCP_EXAMPLES: &str = "\
Examples:
  tmpl-cli mcp
  tmpl-cli --root ~/data mcp

Point an MCP client at the command. It serves until its input closes.";

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
    pub(crate) command: Invocation,
}

impl Cli {
    /// Parse a command line as clap declares it. `help_width` is the width
    /// root help wraps to, `None` off a terminal. The checks clap cannot
    /// express are [`GlobalArgs::check`]'s.
    pub(crate) fn parse_args(
        args: &[OsString],
        help_width: Option<usize>,
    ) -> Result<Self, clap::Error> {
        let matches = root(help_width).try_get_matches_from(args)?;
        Self::from_arg_matches(&matches)
    }
}

/// Arguments to `mcp`: none of its own; `--root` picks the data directory
/// every tool runs against. The server itself is `crate::mcp`.
#[derive(Debug, Args)]
pub(crate) struct McpArgs {}

/// What an invocation does: run one command, or serve every command as a
/// tool.
#[derive(Debug, Subcommand)]
pub(crate) enum Invocation {
    /// One command.
    #[command(flatten)]
    Run(Command),
    /// Serve every command as an MCP tool on stdin and stdout
    #[command(after_help = MCP_EXAMPLES)]
    Mcp(McpArgs),
}

/// Flags accepted before or after any subcommand (STD-01 R4).
#[derive(Debug, Args)]
pub(crate) struct GlobalArgs {
    // The data directory. Its help names the variable from the kit's
    // declaration (STD-01 R25), so no variable name is typed here.
    #[arg(
        long,
        global = true,
        value_name = "DIR",
        help = root_help(),
        help_heading = GLOBAL
    )]
    pub(crate) root: Option<PathBuf>,

    // The output format; its help is derived the same way.
    #[arg(
        long,
        global = true,
        value_enum,
        value_name = "FORMAT",
        help = format_help(),
        help_heading = GLOBAL
    )]
    pub(crate) format: Option<Format>,

    /// Shorthand for --format json [default: false]
    #[arg(long, global = true, help_heading = GLOBAL)]
    pub(crate) json: bool,
}

impl GlobalArgs {
    /// The checks clap cannot express, as the surface's own usage error
    /// (exit 2, through the kit's `Sink::report` like any other failure).
    ///
    /// `conflicts_with` does not see a global flag given at a different
    /// subcommand level (`--json status --format table`), so the
    /// `--json`/`--format` conflict is checked here.
    pub(crate) fn check(&self) -> Result<(), Error> {
        if self.json && self.format.is_some_and(|format| format != Format::Json) {
            return Err(usage_error(
                "--json cannot be used with a --format other than json",
            ));
        }
        Ok(())
    }

    /// The output format the flags ask for, or `None` to defer to the
    /// environment and then `auto`.
    pub(crate) fn requested_format(&self) -> Option<Format> {
        if self.json {
            Some(Format::Json)
        } else {
            self.format
        }
    }
}

/// `--root`'s help: the variable's name comes from its declaration
/// (`tmpl_cli_core::env::ROOT`), the default directory from the program's
/// name (STD-01 R25).
fn root_help() -> String {
    format!(
        "Data directory [default: ${}, else ~/.{APP}]",
        ROOT.name_for(APP)
    )
}

/// `--format`'s help, its variable named from `tmpl_cli_core::env::FORMAT`.
fn format_help() -> String {
    format!(
        "Output format; auto is a table on a terminal, tab-separated lines otherwise, \
         and json is one JSON document [default: ${}, else auto]",
        FORMAT.name_for(APP)
    )
}

/// The command tree with its root help grouped by the kit, wrapped to
/// `help_width` (`None` keeps one line per command).
fn root(help_width: Option<usize>) -> clap::Command {
    grouped(Cli::command(), GROUPS, help_width)
}

/// The format a command line that failed to parse asked for, so the usage
/// error can be reported in it (STD-01 R19). Parsing failed, so this reads
/// the raw arguments up to `--`: `--json`, `--format <value>` or
/// `--format=<value>`, the last one winning. A bare `--format` does not
/// consume `--` or another flag as its value. A value that names no format
/// asks for text; `None` defers to `TMPL_CLI_FORMAT`.
pub(crate) fn raw_requested_format(args: &[OsString]) -> Option<Format> {
    let named = |value: &str| Format::parse(value).unwrap_or(Format::Auto);
    let mut requested = None;
    let mut iter = args
        .iter()
        .skip(1)
        .map(|os_arg| os_arg.to_string_lossy())
        .peekable();
    while let Some(arg) = iter.next() {
        match arg.as_ref() {
            "--" => break,
            "--json" => requested = Some(Format::Json),
            "--format" => {
                let value = iter.next_if(|next| !is_flag(next));
                requested = Some(value.map_or(Format::Auto, |value| named(&value)));
            }
            other => {
                if let Some(value) = other.strip_prefix("--format=") {
                    requested = Some(named(value));
                }
            }
        }
    }
    requested
}

/// Whether `token` looks like `--` or a flag (`-x`, `--json`) rather than a
/// value for `--format`.
fn is_flag(token: &str) -> bool {
    token.starts_with('-') && token.len() > 1
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
    let mut root = root(None);
    root.build();
    let mut out = Vec::new();
    collect(&root, &mut vec![String::from("tmpl-cli")], &mut out);
    out
}
