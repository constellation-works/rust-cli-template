use crate::Store;
use crate::contract::{AuditEvent, AuditStatus};
use crate::error::Error;
use cw_fs::{Error as FsError, Lock};
use std::path::Path;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;
use time::OffsetDateTime;

/// A lock or a write held by another thread until this is dropped, as
/// another process would hold it.
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

/// The kit's file lock at `path`, held by another thread: cw-fs refuses a
/// thread that asks again for a lock it holds (`Reentrant`).
fn hold_lock_elsewhere(path: &Path) -> HeldElsewhere {
    let path = path.to_path_buf();
    held(move |taken, release| {
        let _lock = Lock::acquire(&path, "test", Duration::from_secs(1)).unwrap();
        taken.send(()).unwrap();
        let _ = release.recv();
    })
}

/// A write transaction on the store at `root`, open in another thread.
fn hold_write_elsewhere(root: &Path) -> HeldElsewhere {
    let store = Store::open(root);
    held(move |taken, release| {
        store
            .write(|_tx| {
                taken.send(()).unwrap();
                let _ = release.recv();
                Ok(())
            })
            .unwrap();
    })
}

fn held(hold: impl FnOnce(mpsc::Sender<()>, mpsc::Receiver<()>) + Send + 'static) -> HeldElsewhere {
    let (taken_tx, taken_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let thread = std::thread::spawn(move || hold(taken_tx, release_rx));
    taken_rx.recv().unwrap();
    HeldElsewhere {
        release: Some(release_tx),
        thread: Some(thread),
    }
}

/// One write that changes nothing, as an app's write would begin; `ran`
/// records whether its closure was reached.
fn write(store: &Store, ran: &mut bool) -> Result<(), cw_base::Error> {
    store.write(|_tx| {
        *ran = true;
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
fn a_write_refuses_when_another_writer_holds_the_lock_and_runs_nothing() {
    let dir = crate::tests::private_tempdir();
    let store = Store::open(dir.path()).with_lock_wait(Duration::from_millis(60));
    let held = hold_write_elsewhere(dir.path());
    let mut ran = false;
    let err = write(&store, &mut ran).unwrap_err();
    assert_eq!(err.code(), "store_locked", "got {err:?}");
    assert!(matches!(
        err.downcast_ref::<Error>(),
        Some(Error::Sqlite(cw_sqlite::Error::Locked { .. }))
    ));
    drop(held);
    assert!(!ran, "a refused write runs nothing");
}

#[test]
fn record_audit_appends_one_line_per_event_and_creates_the_directory() {
    let dir = crate::tests::private_tempdir();
    let root = dir.path().join("data");
    let store = Store::open(&root);
    store.audit().append(&event("record add")).unwrap();
    store.audit().append(&event("record add")).unwrap();
    let text = std::fs::read_to_string(store.audit_path()).unwrap();
    assert_eq!(text.lines().count(), 2, "got {text:?}");
    assert!(text.ends_with('\n'));
    assert!(
        !root.join(crate::STORE_FILE).exists(),
        "auditing opens no database"
    );
}

#[test]
fn record_audit_succeeds_while_the_store_lock_is_held() {
    // Defect 1: a command that timed out on the store lock must still write
    // its failure audit line without waiting a second lock deadline.
    let dir = crate::tests::private_tempdir();
    let deadline = Duration::from_millis(80);
    let store = Store::open(dir.path()).with_lock_wait(deadline);
    let _held = hold_write_elsewhere(dir.path());
    let started = std::time::Instant::now();
    let err = write(&store, &mut false).unwrap_err();
    assert_eq!(err.code(), "store_locked");
    store.audit().append(&event("record add")).unwrap();
    let elapsed = started.elapsed();
    assert!(
        elapsed < deadline * 2,
        "write + record_audit waited more than one deadline: {elapsed:?}"
    );
    let text = std::fs::read_to_string(store.audit_path()).unwrap();
    assert_eq!(text.lines().count(), 1, "got {text:?}");
}

#[test]
fn record_audit_waits_for_the_audit_lock() {
    let dir = crate::tests::private_tempdir();
    let store = Store::open(dir.path()).with_lock_wait(Duration::from_millis(60));
    let _held = hold_lock_elsewhere(&dir.path().join(".audit.lock"));
    let err = store.audit().append(&event("record add")).unwrap_err();
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
    let dir = crate::tests::private_tempdir();
    let store = Store::open(dir.path());
    let bad_date = time::Date::from_calendar_date(-1, time::Month::January, 1).unwrap();
    let mut bad_event = event("record add");
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

#[cfg(unix)]
#[test]
fn a_write_through_a_symlinked_root_is_refused_before_any_side_effect() {
    // STD-02 R34: the public write checks the root before it creates the
    // directory or the database, as the audit append does.
    let dir = crate::tests::private_tempdir();
    let target = dir.path().join("target");
    std::fs::create_dir(&target).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let err = write(&Store::open(&link), &mut false).unwrap_err();
    assert_eq!(err.code(), "store_symlink", "got {err:?}");
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
}
