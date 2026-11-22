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

#[test]
fn lock_timeout_audited_command_waits_one_deadline_and_records_one_failure_line() {
    use crate::app::Context;
    use crate::commands::run_with;
    use std::fs::OpenOptions;
    use std::time::{Duration, Instant};
    use tmpl_cli_core::Store;

    let dir = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    // Hold the store lock `.lock` for the duration of the command.
    let lock_path = dir.path().join(".lock");
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .unwrap();
    lock_file.try_lock().unwrap();

    let deadline = Duration::from_millis(80);
    let store = Store::open(dir.path()).with_lock_wait(deadline);
    let ctx = Context::with_store(store, Some(String::from("alice")));
    let cmd = command(&["tmpl-cli", "note", "add", "blocked"]);

    let started = Instant::now();
    let mut audit_written_before_finish = false;
    let err_code = run_with(cmd, &ctx, |outcome| {
        // The audit guard must not have dropped before finish runs.
        audit_written_before_finish = ctx.store().audit_path().exists();
        outcome.unwrap_err().code()
    });
    let elapsed = started.elapsed();

    assert_eq!(err_code, "store_locked");
    assert!(
        !audit_written_before_finish,
        "finish must run before the audit guard drops"
    );
    assert!(
        elapsed >= deadline && elapsed < deadline * 2,
        "expected roughly one deadline ({deadline:?}), took {elapsed:?}"
    );

    let audit_raw = std::fs::read_to_string(ctx.store().audit_path()).unwrap();
    let lines: Vec<serde_json::Value> = audit_raw
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 1, "expected 1 audit line, got {lines:?}");
    assert_eq!(lines[0]["command"], "note add");
    assert_eq!(lines[0]["status"], "failure");
    assert_eq!(lines[0]["error_code"], "store_locked");
    assert_eq!(lines[0]["actor"], "alice");
}
