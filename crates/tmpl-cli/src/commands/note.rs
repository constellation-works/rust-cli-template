//! The `note` noun: its subcommands, their flags and help, and their
//! handlers. A handler maps the parsed arguments to the `note` app's
//! `Command`, calls `apps::note::exec`, and turns the `Outcome` into a payload
//! (STD-02 R2); rendering happens later, in `output`, from the payload alone.
//!
//! A noun this small is one file. When a noun grows (more verbs, or verbs with
//! helpers of their own), it becomes a directory split along the verbs:
//! `note/mod.rs` for `NoteCommand` and dispatch, `note/add.rs`,
//! `note/list.rs`, `note/show.rs` each holding a verb's `Args` and handler,
//! `note/support.rs` for what they share, and `note/tests/` beside them.
//! STD-02 R18: split along responsibilities, not by length alone.

use crate::error::CliError;
use crate::output::{NotePayload, Output};
use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::{ArgAction, Args, Subcommand};
use std::num::NonZeroUsize;
use tmpl_cli_core::Env;
use tmpl_cli_core::apps::note::{self, ListFilter, NewNote, NoteList, Outcome};
use tmpl_cli_core::contracts::{Body, NoteId, Priority, Tag, Title};

pub(super) const NOTE_EXAMPLES: &str = "\
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

    /// Label the note; repeat for several (lowercase letters, digits, '-') [default: none]
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

/// Run one `note` verb through the `note` app and pass the payload to
/// `finish`.
pub(super) fn run<R>(
    command: NoteCommand,
    env: &Env,
    finish: impl FnOnce(Result<Output, CliError>) -> R,
) -> R {
    note::exec(env, app_command(command), |outcome| {
        finish(outcome.map(output).map_err(CliError::from))
    })
}

/// The parsed arguments as the app's command.
fn app_command(command: NoteCommand) -> note::Command {
    match command {
        NoteCommand::Add(args) => note::Command::Add(NewNote {
            title: args.title,
            body: args.body,
            tags: args.tags,
            priority: args.priority.unwrap_or_default(),
        }),
        NoteCommand::List(args) => note::Command::List(ListFilter {
            tag: args.tag,
            priority: args.priority,
            limit: args.limit,
        }),
        NoteCommand::Show(args) => note::Command::Show(args.id),
    }
}

/// The app's outcome as the payload every output mode renders.
fn output(outcome: Outcome) -> Output {
    match outcome {
        Outcome::Added { note, path } => {
            // Name what was written and where (STD-01 R30); `id` is in the payload.
            let notice = format!("added note {} to {}", note.id, path.display());
            Output::Note {
                note: NotePayload::from(&note),
                notice: Some(notice),
                warnings: Vec::new(),
            }
        }
        Outcome::Listed {
            list,
            filter,
            bad_records,
        } => Output::Notes {
            notes: list.notes.iter().map(NotePayload::from).collect(),
            total: list.total,
            notice: list_notice(&filter, &list),
            warnings: bad_records.iter().map(ToString::to_string).collect(),
        },
        Outcome::Shown { note, bad_records } => Output::Note {
            note: NotePayload::from(&note),
            notice: None,
            warnings: bad_records.iter().map(ToString::to_string).collect(),
        },
    }
}

/// The stderr line for a list, if it needs one: nothing matched (STD-01
/// R16), or the limit cut the list (R34). A full, non-empty list has none.
pub(super) fn list_notice(filter: &ListFilter, listed: &NoteList) -> Option<String> {
    if listed.total == 0 {
        Some(empty_notice(filter))
    } else if listed.truncated() {
        Some(format!(
            "showing {} of {} matching notes; raise or drop --limit to see the rest",
            listed.notes.len(),
            listed.total
        ))
    } else {
        None
    }
}

/// The line for an empty result, naming what was searched (STD-01 R16).
fn empty_notice(filter: &ListFilter) -> String {
    let mut criteria = Vec::new();
    if let Some(tag) = &filter.tag {
        criteria.push(format!("--tag {}", tag.as_str()));
    }
    if let Some(priority) = filter.priority {
        criteria.push(format!("--priority {priority}"));
    }
    if criteria.is_empty() {
        String::from("no notes yet; add one with: tmpl-cli note add <title>")
    } else {
        format!("no notes match {}", criteria.join(" "))
    }
}
