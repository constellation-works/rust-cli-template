use crate::audit_middleware::{AuditGuard, AuditMeta};
use crate::error::CliError;
use crate::output::{NotePayload, Output};
use std::path::PathBuf;
use time::macros::datetime;
use tmpl_cli_core::{AuditStatus, NewNote, Priority, Store};

const ADD: AuditMeta = AuditMeta {
    command: "note add",
};

/// An owner-only temporary directory, as a store directory must be.
fn private_tempdir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    dir
}

fn lines(store: &Store) -> Vec<serde_json::Value> {
    std::fs::read_to_string(store.audit_path())
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn added_note(store: &Store) -> Output {
    let new = NewNote {
        title: "secret title".parse().unwrap(),
        body: None,
        tags: Vec::new(),
        priority: Priority::Normal,
    };
    let note = store.add(new, datetime!(2026-09-26 12:00:00 UTC)).unwrap();
    Output::Note {
        note: NotePayload::from(&note),
        notice: None,
    }
}

#[test]
fn an_unmarked_guard_records_a_failure() {
    // A command that returned early or panicked never reads as success.
    let dir = private_tempdir();
    let store = Store::open(dir.path());
    let guard = AuditGuard::start(&store, ADD, datetime!(2026-09-26 12:00:00 UTC));
    assert_eq!(guard.event().status, AuditStatus::Failure);
    drop(guard);
    let lines = lines(&store);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["status"], "failure");
    assert_eq!(lines[0]["command"], "note add");
    assert_eq!(lines[0]["error_code"], serde_json::Value::Null);
}

#[test]
fn a_success_records_the_note_it_acted_on_and_nothing_it_contained() {
    let dir = private_tempdir();
    let store = Store::open(dir.path());
    let output = added_note(&store);
    let mut guard = AuditGuard::start(&store, ADD, datetime!(2026-09-26 12:00:00 UTC));
    guard.mark_result(&Ok(output));
    drop(guard);
    let lines = lines(&store);
    assert_eq!(lines[0]["status"], "success");
    assert_eq!(lines[0]["target"], "1");
    assert_eq!(lines[0]["at"], "2026-09-26T12:00:00Z");
    // STD-05 R13: no argument values in the log.
    let text = std::fs::read_to_string(store.audit_path()).unwrap();
    assert!(!text.contains("secret title"), "got {text}");
}

#[test]
fn a_failure_records_the_stable_code_never_the_message() {
    let dir = private_tempdir();
    let store = Store::open(dir.path());
    let err = CliError::Core(store.show("9".parse().unwrap()).unwrap_err());
    let message = err.to_string();
    let mut guard = AuditGuard::start(&store, ADD, datetime!(2026-09-26 12:00:00 UTC));
    guard.mark_result(&Err(err));
    drop(guard);
    let lines = lines(&store);
    assert_eq!(lines[0]["status"], "failure");
    assert_eq!(lines[0]["error_code"], "note_not_found");
    assert_eq!(lines[0]["target"], serde_json::Value::Null);
    let text = std::fs::read_to_string(store.audit_path()).unwrap();
    assert!(!text.contains(&message), "the message names a path: {text}");
}

#[test]
fn an_unwritable_audit_log_fails_open() {
    // STD-02 R31: the data directory is a regular file, so the line cannot be
    // written; dropping the guard must neither panic nor fail the command.
    let dir = private_tempdir();
    let file = dir.path().join("not-a-dir");
    std::fs::write(&file, b"").unwrap();
    let store = Store::open(PathBuf::from(&file));
    let mut guard = AuditGuard::start(&store, ADD, datetime!(2026-09-26 12:00:00 UTC));
    guard.mark_result(&Err(CliError::NoDataDir));
    drop(guard);
    assert_eq!(std::fs::read(&file).unwrap(), b"");
}
