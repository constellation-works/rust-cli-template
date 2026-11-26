#![expect(
    clippy::disallowed_methods,
    reason = "a test reads the data directory back and removes the database's sidecars"
)]

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

/// Fold the log into the main file, so it alone holds every record.
fn checkpoint(store: &Store) {
    crate::driver::sqlite::open(store.root(), Duration::from_secs(5))
        .checkpoint_truncate()
        .unwrap();
}

/// Remove the `-wal` and `-shm` beside the database, as copying only its
/// main file would leave them.
fn remove_sidecars(store: &Store) {
    for suffix in ["-wal", "-shm"] {
        let mut name = store.path().into_os_string();
        name.push(suffix);
        match std::fs::remove_file(&name) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => panic!("remove {name:?}: {err}"),
        }
    }
}

/// Every entry in `root` with its bytes (empty for a directory) and
/// modification time.
fn state_of(root: &Path) -> Vec<(String, Vec<u8>, std::time::SystemTime)> {
    let mut files: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let meta = entry.metadata().unwrap();
            let bytes = if meta.is_file() {
                std::fs::read(entry.path()).unwrap()
            } else {
                Vec::new()
            };
            (
                entry.file_name().to_string_lossy().into_owned(),
                bytes,
                meta.modified().unwrap(),
            )
        })
        .collect();
    files.sort();
    files
}

#[test]
fn a_read_creates_rebuilds_and_changes_no_file() {
    // STD-01 R31: an observation writes nothing, not even the index.
    let dir = crate::tests::private_tempdir();
    let store = Store::open(dir.path());
    store.write(|_tx| Ok(())).unwrap();
    let before = state_of(dir.path());
    assert!(
        before.iter().any(|(name, ..)| name == "store.db-shm"),
        "the write left its index beside the database"
    );
    for _ in 0..2 {
        let snapshot = store.read().unwrap();
        assert_eq!(snapshot.format(), Some(1));
    }
    assert_eq!(state_of(dir.path()), before);
}

#[test]
fn a_read_of_a_database_without_its_wal_or_shm_is_refused_and_the_next_write_repairs_it() {
    let dir = crate::tests::private_tempdir();
    let store = Store::open(dir.path());
    store.write(|_tx| Ok(())).unwrap();
    checkpoint(&store);
    remove_sidecars(&store);
    let before = state_of(dir.path());

    let err = store.read().unwrap_err();
    assert_eq!(err.code(), "store_wal_unavailable", "got {err:?}");
    assert!(matches!(
        err.downcast_ref::<Error>(),
        Some(Error::Sqlite(cw_sqlite::Error::WalUnavailable { .. }))
    ));
    assert_eq!(
        state_of(dir.path()),
        before,
        "a refused read creates nothing"
    );

    let mut ran = false;
    write(&store, &mut ran).unwrap();
    assert!(ran);
    assert_eq!(store.read().unwrap().format(), Some(1));
}

#[test]
fn the_read_before_a_write_does_not_refuse_a_database_without_its_wal_or_shm() {
    let dir = crate::tests::private_tempdir();
    let store = Store::open(dir.path());
    store.write(|_tx| Ok(())).unwrap();
    checkpoint(&store);
    remove_sidecars(&store);
    assert_eq!(store.read_before_write().unwrap().format(), Some(1));
}
