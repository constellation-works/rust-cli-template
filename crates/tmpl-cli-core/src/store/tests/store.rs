use crate::error::Error;
use crate::note::{NewNote, Priority};
use crate::store::fsio::LockGuard;
use crate::store::{AuditEvent, AuditStatus, Store};
use std::time::Duration;
use time::OffsetDateTime;

fn new_note(title: &str) -> NewNote {
    NewNote {
        title: title.parse().unwrap(),
        body: None,
        tags: Vec::new(),
        priority: Priority::Normal,
    }
}

fn event(command: &str) -> AuditEvent {
    AuditEvent {
        at: OffsetDateTime::UNIX_EPOCH,
        command: command.to_owned(),
        status: AuditStatus::Success,
        target: None,
        error_code: None,
        duration_ms: 0,
    }
}

#[test]
fn add_refuses_when_another_writer_holds_the_lock_and_writes_nothing() {
    let dir = super::private_tempdir();
    let store = Store::open(dir.path()).with_lock_wait(Duration::from_millis(60));
    let _held =
        LockGuard::acquire(&dir.path().join(".lock"), "test", Duration::from_secs(1)).unwrap();
    let err = store
        .add(new_note("blocked"), OffsetDateTime::UNIX_EPOCH)
        .unwrap_err();
    assert_eq!(err.code(), "store_locked", "got {err:?}");
    assert!(matches!(err, Error::Locked { .. }));
    assert!(!dir.path().join(crate::STORE_FILE).exists());
}

#[test]
fn record_audit_appends_one_line_per_event_and_creates_the_directory() {
    let dir = super::private_tempdir();
    let root = dir.path().join("data");
    let store = Store::open(&root);
    store.record_audit(&event("note add")).unwrap();
    store.record_audit(&event("note add")).unwrap();
    let text = std::fs::read_to_string(store.audit_path()).unwrap();
    assert_eq!(text.lines().count(), 2, "got {text:?}");
    assert!(text.ends_with('\n'));
    assert!(
        !root.join(crate::STORE_FILE).exists(),
        "auditing writes no notes"
    );
}

#[test]
fn record_audit_waits_for_the_store_lock() {
    // Events and note writes share one lock, so a log line is never written
    // while another command is mid-write.
    let dir = super::private_tempdir();
    let store = Store::open(dir.path()).with_lock_wait(Duration::from_millis(60));
    let _held =
        LockGuard::acquire(&dir.path().join(".lock"), "test", Duration::from_secs(1)).unwrap();
    let err = store.record_audit(&event("note add")).unwrap_err();
    assert!(matches!(err, Error::Locked { .. }), "got {err:?}");
    assert!(!store.audit_path().exists());
}
