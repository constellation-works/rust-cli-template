use crate::Store;
use crate::contracts::{AuditEvent, AuditStatus, Note, NoteId, Priority};
use crate::error::Error;
use cw_fs::{Error as FsError, Lock};
use std::path::Path;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;
use time::OffsetDateTime;

/// A lock held by another thread until this is dropped: cw-fs refuses a
/// thread that asks again for a lock it holds (`Reentrant`), so the other
/// writer must be another thread, as it would be another process.
struct HeldElsewhere {
    release: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for HeldElsewhere {
    fn drop(&mut self) {
        drop(self.release.take());
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

fn hold_elsewhere(path: &Path) -> HeldElsewhere {
    let path = path.to_path_buf();
    let (taken_tx, taken_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let thread = std::thread::spawn(move || {
        let _lock = Lock::acquire(&path, "test", Duration::from_secs(1)).unwrap();
        taken_tx.send(()).unwrap();
        let _ = release_rx.recv();
    });
    taken_rx.recv().unwrap();
    HeldElsewhere {
        release: Some(release_tx),
        thread: Some(thread),
    }
}

fn new_note(title: &str) -> Note {
    Note {
        id: NoteId::FIRST,
        title: title.parse().unwrap(),
        body: None,
        tags: Vec::new(),
        priority: Priority::Normal,
        created_at: OffsetDateTime::UNIX_EPOCH,
    }
}

/// Store one note in one write, as the note app's add does.
fn add(store: &Store, note: Note) -> Result<(), cw_base::Error> {
    store.write("note add", |tx| {
        tx.notes().put(note);
        Ok(())
    })
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
    let dir = crate::db::tests::private_tempdir();
    let store = Store::open(dir.path()).with_lock_wait(Duration::from_millis(60));
    let _held = hold_elsewhere(&dir.path().join(".lock"));
    let err = add(&store, new_note("blocked")).unwrap_err();
    assert_eq!(err.code(), "store_locked", "got {err:?}");
    assert!(matches!(
        err.downcast_ref::<Error>(),
        Some(Error::Fs(FsError::Locked { .. }))
    ));
    assert!(!dir.path().join(crate::STORE_FILE).exists());
}

#[test]
fn record_audit_appends_one_line_per_event_and_creates_the_directory() {
    let dir = crate::db::tests::private_tempdir();
    let root = dir.path().join("data");
    let store = Store::open(&root);
    store.audit().append(&event("note add")).unwrap();
    store.audit().append(&event("note add")).unwrap();
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
    // its failure audit line without waiting a second lock deadline.
    let dir = crate::db::tests::private_tempdir();
    let deadline = Duration::from_millis(80);
    let store = Store::open(dir.path()).with_lock_wait(deadline);
    let _held = hold_elsewhere(&dir.path().join(".lock"));
    let started = std::time::Instant::now();
    let err = add(&store, new_note("blocked")).unwrap_err();
    assert_eq!(err.code(), "store_locked");
    store.audit().append(&event("note add")).unwrap();
    let elapsed = started.elapsed();
    assert!(
        elapsed < deadline * 2,
        "add + record_audit waited more than one deadline: {elapsed:?}"
    );
    let text = std::fs::read_to_string(store.audit_path()).unwrap();
    assert_eq!(text.lines().count(), 1, "got {text:?}");
}

#[test]
fn record_audit_waits_for_the_audit_lock() {
    let dir = crate::db::tests::private_tempdir();
    let store = Store::open(dir.path()).with_lock_wait(Duration::from_millis(60));
    let _held = hold_elsewhere(&dir.path().join(".audit.lock"));
    let err = store.audit().append(&event("note add")).unwrap_err();
    assert!(
        matches!(
            err.downcast_ref::<Error>(),
            Some(Error::Fs(FsError::Locked { .. }))
        ),
        "got {err:?}"
    );
    assert!(!store.audit_path().exists());
}

#[test]
fn record_audit_encode_failure_is_not_io_and_names_no_path() {
    let dir = crate::db::tests::private_tempdir();
    let store = Store::open(dir.path());
    let bad_date = time::Date::from_calendar_date(-1, time::Month::January, 1).unwrap();
    let mut bad_event = event("note add");
    bad_event.at = OffsetDateTime::new_utc(bad_date, time::Time::MIDNIGHT);
    let err = store.audit().append(&bad_event).unwrap_err();
    assert!(
        matches!(err.downcast_ref::<Error>(), Some(Error::Encode { .. })),
        "got {err:?}"
    );
    assert_eq!(err.code(), "encode_error");
    let msg = err.to_string();
    assert!(
        !msg.contains(&dir.path().display().to_string()) && !msg.contains("audit.jsonl"),
        "encode error must not name an untouched path: {msg}"
    );
}
