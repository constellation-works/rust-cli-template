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
        actor: None,
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
fn record_audit_succeeds_while_the_store_lock_is_held() {
    // Defect 1: a command that timed out on the store lock must still write
    // its failure audit line without waiting on the store lock a second time.
    let dir = super::private_tempdir();
    let store = Store::open(dir.path()).with_lock_wait(Duration::ZERO);
    let _held =
        LockGuard::acquire(&dir.path().join(".lock"), "test", Duration::from_secs(1)).unwrap();
    let err = store
        .add(new_note("blocked"), OffsetDateTime::UNIX_EPOCH)
        .unwrap_err();
    assert_eq!(err.code(), "store_locked");
    assert_eq!(store.contended_lock_waits(), 1);
    store.record_audit(&event("note add")).unwrap();
    assert_eq!(
        store.contended_lock_waits(),
        1,
        "record_audit must not contend on the held store lock"
    );
    let text = std::fs::read_to_string(store.audit_path()).unwrap();
    assert_eq!(text.lines().count(), 1, "got {text:?}");
}

#[test]
fn record_audit_waits_for_the_audit_lock() {
    let dir = super::private_tempdir();
    let store = Store::open(dir.path()).with_lock_wait(Duration::ZERO);
    let _held = LockGuard::acquire(
        &dir.path().join(".audit.lock"),
        "test",
        Duration::from_secs(1),
    )
    .unwrap();
    let err = store.record_audit(&event("note add")).unwrap_err();
    assert!(matches!(err, Error::Locked { .. }), "got {err:?}");
    assert_eq!(store.contended_lock_waits(), 1);
    assert!(!store.audit_path().exists());
}

#[test]
fn record_audit_encode_failure_is_not_io_and_names_no_path() {
    let dir = super::private_tempdir();
    let store = Store::open(dir.path());
    let bad_date = time::Date::from_calendar_date(-1, time::Month::January, 1).unwrap();
    let mut bad_event = event("note add");
    bad_event.at = OffsetDateTime::new_utc(bad_date, time::Time::MIDNIGHT);
    let err = store.record_audit(&bad_event).unwrap_err();
    assert!(matches!(err, Error::Encode { .. }), "got {err:?}");
    assert_eq!(err.code(), "encode_error");
    let msg = err.to_string();
    assert!(
        !msg.contains(&dir.path().display().to_string()) && !msg.contains("audit.jsonl"),
        "encode error must not name an untouched path: {msg}"
    );
}
