use crate::Store;
use crate::error::Error;
use std::path::Path;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

/// A write held by another thread until this is dropped, as
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

#[cfg(unix)]
#[test]
fn a_write_through_a_symlinked_root_is_refused_before_any_side_effect() {
    // STD-02 R34: the public write checks the root before it creates the
    // directory or the database, as a read does.
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
