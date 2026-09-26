//! Command dispatch: parsed arguments in, a payload out (STD-02 R2). Each
//! verb makes one store call and wraps the result; rendering happens later,
//! in `output`, from the payload alone.

use crate::app::Context;
use crate::cli::{AddArgs, Command, ListArgs, NoteCommand, ShowArgs};
use crate::error::CliError;
use crate::output::{NotePayload, Output};
use tmpl_cli_core::{ListFilter, NewNote, NoteList};

/// Run one command.
pub(crate) fn run(command: Command, ctx: &Context) -> Result<Output, CliError> {
    match command {
        Command::Note(NoteCommand::Add(args)) => add(args, ctx),
        Command::Note(NoteCommand::List(args)) => list(args, ctx),
        Command::Note(NoteCommand::Show(args)) => show(&args, ctx),
    }
}

fn add(args: AddArgs, ctx: &Context) -> Result<Output, CliError> {
    let new = NewNote {
        title: args.title,
        body: args.body,
        tags: args.tags,
        priority: args.priority.unwrap_or_default(),
    };
    let note = ctx.store().add(new, ctx.now())?;
    tracing::info!(id = %note.id, "note added");
    // Name what was written and where (STD-01 R30); `id` is in the payload.
    let notice = format!("added note {} to {}", note.id, ctx.store().path().display());
    Ok(Output::Note {
        note: NotePayload::from(&note),
        notice: Some(notice),
    })
}

fn list(args: ListArgs, ctx: &Context) -> Result<Output, CliError> {
    let filter = ListFilter {
        tag: args.tag,
        priority: args.priority,
        limit: args.limit,
    };
    let listed = ctx.store().list(&filter)?;
    Ok(Output::Notes {
        notes: listed.notes.iter().map(NotePayload::from).collect(),
        total: listed.total,
        notice: list_notice(&filter, &listed),
    })
}

fn show(args: &ShowArgs, ctx: &Context) -> Result<Output, CliError> {
    let note = ctx.store().show(args.id)?;
    Ok(Output::Note {
        note: NotePayload::from(&note),
        notice: None,
    })
}

/// The stderr line for a list, if it needs one: nothing matched (STD-01
/// R16), or the limit cut the list (R34). A full, non-empty list has none.
pub(crate) fn list_notice(filter: &ListFilter, listed: &NoteList) -> Option<String> {
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
