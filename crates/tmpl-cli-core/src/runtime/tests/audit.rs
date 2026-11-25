use crate::composition::Error;
use crate::runtime::{AuditGuard, AuditMeta};
use std::path::PathBuf;
use time::macros::datetime;
use tmpl_cli_store::Store;
use tmpl_cli_store::contract::AuditStatus;

const ADD: AuditMeta = AuditMeta {
    command: "record add",
};

/// An owner-only temporary directory, as a store directory must be.
fn private_tempdir() -> cw_testkit::TempRoot {
    cw_testkit::TempRoot::new("TMPL_CLI").unwrap()
}

fn lines(store: &Store) -> Vec<serde_json::Value> {
    std::fs::read_to_string(store.audit_path())
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn an_unmarked_guard_records_a_failure() {
    // A command that returned early or panicked never reads as success.
    let dir = private_tempdir();
    let store = Store::open(dir.path());
    let guard = AuditGuard::start(&store, ADD, datetime!(2026-09-26 12:00:00 UTC), None);
    assert_eq!(guard.event().status, AuditStatus::Failure);
    drop(guard);
    let lines = lines(&store);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["status"], "failure");
    assert_eq!(lines[0]["command"], "record add");
    assert_eq!(lines[0]["actor"], serde_json::Value::Null);
    assert_eq!(lines[0]["error_code"], serde_json::Value::Null);
}

#[test]
fn a_success_records_the_actor_and_the_record_it_acted_on() {
    let dir = private_tempdir();
    let store = Store::open(dir.path());
    let mut guard = AuditGuard::start(
        &store,
        ADD,
        datetime!(2026-09-26 12:00:00 UTC),
        Some(String::from("alice")),
    );
    guard.mark_result(Ok(Some(String::from("7"))));
    drop(guard);
    let lines = lines(&store);
    assert_eq!(lines[0]["status"], "success");
    assert_eq!(lines[0]["actor"], "alice");
    assert_eq!(lines[0]["target"], "7");
    assert_eq!(lines[0]["at"], "2026-09-26T12:00:00Z");
}

#[test]
fn a_failure_records_the_stable_code_never_the_message() {
    let dir = private_tempdir();
    let store = Store::open(dir.path());
    let err = Error::NoDataDir;
    let message = err.to_string();
    let mut guard = AuditGuard::start(&store, ADD, datetime!(2026-09-26 12:00:00 UTC), None);
    guard.mark_result(Err(err.code()));
    drop(guard);
    let lines = lines(&store);
    assert_eq!(lines[0]["status"], "failure");
    assert_eq!(lines[0]["error_code"], "no_data_dir");
    assert_eq!(lines[0]["target"], serde_json::Value::Null);
    let text = std::fs::read_to_string(store.audit_path()).unwrap();
    assert!(
        !text.contains(&message),
        "the line carries the message: {text}"
    );
}

#[test]
fn an_unwritable_audit_log_fails_open() {
    // STD-02 R31: the data directory is a regular file, so the line cannot be
    // written; dropping the guard must neither panic nor fail the command.
    let dir = private_tempdir();
    let file = dir.path().join("not-a-dir");
    std::fs::write(&file, b"").unwrap();
    let store = Store::open(PathBuf::from(&file));
    let mut guard = AuditGuard::start(&store, ADD, datetime!(2026-09-26 12:00:00 UTC), None);
    guard.mark_result(Err(Error::NoDataDir.code()));
    drop(guard);
    assert_eq!(std::fs::read(&file).unwrap(), b"");
}
