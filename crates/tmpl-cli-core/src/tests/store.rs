use crate::error::Error;
use crate::fsio::LockGuard;
use crate::note::{NewNote, Priority};
use crate::store::{FORMAT, Store, UPGRADES};
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
fn the_format_is_one_more_than_the_number_of_upgrade_steps() {
    // STD-03 R23: a format bump without its upgrade step, or the reverse,
    // fails here rather than stranding older stores.
    assert_eq!(usize::try_from(FORMAT).unwrap(), UPGRADES.len() + 1);
}
