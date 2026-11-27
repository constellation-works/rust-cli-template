use crate::Store;
use crate::error::Error;
use std::path::Path;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

/// A write held by another thread until this is dropped, as
/// another process would hold it.
struct HeldElsewhere {
    release: Option<mpsc::SyncSender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for HeldElsewhere {
    fn drop(&mut self) {
        drop(self.release.take());
        if let Some(thread) = self.thread.take() {
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            while !thread.is_finished() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "holder thread timed out"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
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
                let _ = release.recv_timeout(Duration::from_secs(30));
                Ok(())
            })
            .unwrap();
    })
}

fn held(
    hold: impl FnOnce(mpsc::SyncSender<()>, mpsc::Receiver<()>) + Send + 'static,
) -> HeldElsewhere {
    let (taken_tx, taken_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel::<()>(1);
    let thread = std::thread::spawn(move || hold(taken_tx, release_rx));
    taken_rx.recv_timeout(Duration::from_secs(30)).unwrap();
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
    let dir = private_tempdir();
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

/// A temporary directory that is owner-only whatever the umask, as a store
/// directory must be (STD-05 R9). The kit's temp root is `0700`.
fn private_tempdir() -> cw_testkit::TempRoot {
    cw_testkit::TempRoot::new("TMPL_CLI").unwrap()
}
