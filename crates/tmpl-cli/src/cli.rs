//! The command tree: every command, flag and help string, declared once
//! (STD-01 R25). `--help` is rendered from these doc comments, so they are
//! user-facing text: placeholders like `<id>`, never real ids or tracker
//! numbers (STD-01 R23).

use crate::output::FormatArg;
use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::error::ErrorKind;
use clap::{ArgAction, Args, CommandFactory, Parser, Subcommand};
use std::ffi::OsString;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use tmpl_cli_core::{Body, NoteId, Priority, Tag, Title};

/// Help heading for the flags every command accepts.
const GLOBAL: &str = "Global options";

const ROOT_EXAMPLES: &str = "\
Examples:
  tmpl-cli note add \"Call the plumber\" --tag home --priority high
  tmpl-cli note list --tag home
  tmpl-cli note show <id> --json
  tmpl-cli note list | cut -f1,3

Exit status: 0 on success, 1 when a command fails, 2 on a usage error.";

const NOTE_EXAMPLES: &str = "\
Examples:
  tmpl-cli note add \"Renew passport\" --tag admin
  tmpl-cli note list --priority high
  tmpl-cli note show <id>";

const ADD_EXAMPLES: &str = "\
Examples:
  tmpl-cli note add \"Buy milk\"
  tmpl-cli note add \"Draft the report\" --tag work --tag writing --priority high
  tmpl-cli note add \"Ideas\" --body \"longer text, shown by note show\" --json";

const LIST_EXAMPLES: &str = "\
Examples:
  tmpl-cli note list
  tmpl-cli note list --tag work --limit 10
  tmpl-cli note list --json | jq '.notes[].title'
  tmpl-cli note list | wc -l";

const SHOW_EXAMPLES: &str = "\
Examples:
  tmpl-cli note show <id>
  tmpl-cli note show <id> --json";

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

/// Top-level nouns.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Add, list and show notes
    #[command(subcommand, after_help = NOTE_EXAMPLES)]
    Note(NoteCommand),
}

/// Verbs on the `note` noun.
#[derive(Debug, Subcommand)]
pub(crate) enum NoteCommand {
    /// Add a note and print it
    #[command(after_help = ADD_EXAMPLES)]
    Add(AddArgs),
    /// List notes, oldest first
    #[command(after_help = LIST_EXAMPLES)]
    List(ListArgs),
    /// Show one note in full, including its body
    #[command(after_help = SHOW_EXAMPLES)]
    Show(ShowArgs),
}

/// Arguments to `note add`.
#[derive(Debug, Args)]
pub(crate) struct AddArgs {
    /// The note's title: one line, at most 200 characters
    #[arg(value_name = "TITLE")]
    pub(crate) title: Title,

    /// Longer text, shown by `note show` and in --json output; not blank [default: none]
    #[arg(long, value_name = "TEXT")]
    pub(crate) body: Option<Body>,

    /// Label the note; repeat for several (lowercase letters, digits, '-')
    #[arg(long = "tag", value_name = "TAG", action = ArgAction::Append)]
    pub(crate) tags: Vec<Tag>,

    /// How much it matters [default: normal]
    #[arg(long, value_name = "PRIORITY", value_parser = priority_parser())]
    pub(crate) priority: Option<Priority>,
}

/// Arguments to `note list`.
#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    /// Only notes with this tag [default: any]
    #[arg(long, value_name = "TAG")]
    pub(crate) tag: Option<Tag>,

    /// Only notes at this priority [default: any]
    #[arg(long, value_name = "PRIORITY", value_parser = priority_parser())]
    pub(crate) priority: Option<Priority>,

    /// Show at most this many of the matching notes, 1 or more; --json reports the total and whether the list was cut [default: all]
    #[arg(long, value_name = "N")]
    pub(crate) limit: Option<NonZeroUsize>,
}

/// Arguments to `note show`.
#[derive(Debug, Args)]
pub(crate) struct ShowArgs {
    /// The note's id, as printed by `note list`
    #[arg(value_name = "ID")]
    pub(crate) id: NoteId,
}

/// Allowed priorities come from the domain type, so help and parsing cannot
/// drift from it (STD-01 R25).
fn priority_parser() -> impl TypedValueParser<Value = Priority> {
    PossibleValuesParser::new(Priority::NAMES).try_map(|s| s.parse::<Priority>())
}
