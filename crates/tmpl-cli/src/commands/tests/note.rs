use crate::cli::Cli;
use crate::commands::note::list_notice;
use crate::commands::{Command, NoteCommand};
use clap::Parser;
use std::num::NonZeroUsize;
use time::OffsetDateTime;
use tmpl_cli_core::{ListFilter, Note, NoteId, NoteList, Priority};

fn listed(shown: u64, total: usize) -> NoteList {
    let notes = (1..=shown)
        .map(|id| Note {
            id: NoteId::try_from(id).unwrap(),
            title: format!("note {id}").parse().unwrap(),
            body: None,
            tags: Vec::new(),
            priority: Priority::Normal,
            created_at: OffsetDateTime::UNIX_EPOCH,
        })
        .collect();
    NoteList { notes, total }
}

#[test]
fn the_empty_notice_names_every_filter_that_was_applied() {
    // STD-01 R16.
    let filter = ListFilter {
        tag: Some("home".parse().unwrap()),
        priority: Some(Priority::High),
        limit: None,
    };
    assert_eq!(
        list_notice(&filter, &listed(0, 0)).as_deref(),
        Some("no notes match --tag home --priority high")
    );
}

#[test]
fn the_empty_notice_without_filters_says_how_to_add_one() {
    let notice = list_notice(&ListFilter::default(), &listed(0, 0)).unwrap();
    assert!(notice.starts_with("no notes yet"), "got {notice:?}");
    assert!(!notice.contains('\n'), "must be one line");
}

#[test]
fn a_cut_list_says_how_many_matched_and_a_whole_one_says_nothing() {
    // STD-01 R34: the stderr half of the truncation signal.
    let filter = ListFilter {
        limit: NonZeroUsize::new(2),
        ..ListFilter::default()
    };
    let notice = list_notice(&filter, &listed(2, 5)).unwrap();
    assert!(notice.contains("2 of 5"), "got {notice:?}");
    assert_eq!(list_notice(&filter, &listed(2, 2)), None);
}

fn command(args: &[&str]) -> Command {
    Cli::try_parse_from(args).unwrap().command
}

#[test]
fn only_note_add_is_audited() {
    // STD-01 R31: list and show are read-only, so they write no audit line.
    let add = command(&["tmpl-cli", "note", "add", "t"]).audit().unwrap();
    assert_eq!(add.command, "note add");
    assert!(command(&["tmpl-cli", "note", "list"]).audit().is_none());
    assert!(
        command(&["tmpl-cli", "note", "show", "1"])
            .audit()
            .is_none()
    );
}

#[test]
fn the_parsed_verb_reaches_its_noun() {
    assert!(matches!(
        command(&["tmpl-cli", "note", "list"]),
        Command::Note(NoteCommand::List(_))
    ));
}
