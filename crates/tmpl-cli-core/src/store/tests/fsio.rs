use crate::error::Error;
use crate::store::fsio::{LockGuard, write_atomic};
use std::time::Duration;

#[test]
fn write_atomic_replaces_the_file_and_leaves_no_temp_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.json");
    write_atomic(&path, b"one").unwrap();
    write_atomic(&path, b"two").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"two");
    let entries = std::fs::read_dir(dir.path()).unwrap().count();
    assert_eq!(entries, 1, "only the target file should remain");
}

#[test]
fn write_atomic_into_a_missing_directory_fails_without_creating_anything() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing").join("data.json");
    let err = write_atomic(&path, b"x").unwrap_err();
    assert!(matches!(err, Error::Io { .. }), "got {err:?}");
    assert!(!dir.path().join("missing").exists());
}

#[test]
fn a_second_lock_times_out_and_names_the_holder() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".lock");
    let _held = LockGuard::acquire(&path, "first writer", Duration::from_secs(1)).unwrap();
    let err = LockGuard::acquire(&path, "second writer", Duration::from_millis(60)).unwrap_err();
    let Error::Locked { holder, .. } = err else {
        panic!("expected Locked, got {err:?}");
    };
    assert!(
        holder.contains("first writer") && holder.contains(&std::process::id().to_string()),
        "holder should name the label and pid, got {holder:?}"
    );
}

#[test]
fn dropping_the_guard_releases_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".lock");
    drop(LockGuard::acquire(&path, "first", Duration::from_secs(1)).unwrap());
    let again = LockGuard::acquire(&path, "second", Duration::from_millis(60));
    assert!(again.is_ok(), "lock should be free after drop: {again:?}");
}

#[cfg(unix)]
mod private_state {
    //! STD-05 R7–R9: owner-only creation, and refusal of links, foreign
    //! owners and loose modes on load.
    use crate::error::Error;
    use crate::store::fsio::{
        LockGuard, append_private, create_private_dir, inspect_dir, inspect_file, write_atomic,
    };
    use std::fs::{self, Permissions};
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::Path;
    use std::time::Duration;

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o7777
    }

    #[test]
    fn directories_and_files_are_created_owner_only() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a").join("b");
        create_private_dir(&root).unwrap();
        assert_eq!(mode(&dir.path().join("a")), 0o700);
        assert_eq!(mode(&root), 0o700);
        write_atomic(&root.join("data.json"), b"{}").unwrap();
        assert_eq!(mode(&root.join("data.json")), 0o600);
        drop(LockGuard::acquire(&root.join(".lock"), "t", Duration::from_secs(1)).unwrap());
        assert_eq!(mode(&root.join(".lock")), 0o600);
    }

    #[test]
    fn missing_state_is_absent_not_an_error() {
        let dir = crate::store::tests::private_tempdir();
        assert!(!inspect_dir(&dir.path().join("none")).unwrap());
        assert!(!inspect_file(&dir.path().join("none.json")).unwrap());
        assert!(inspect_dir(dir.path()).unwrap());
    }

    #[test]
    fn a_file_writable_by_others_is_refused_with_its_chmod_fix() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, b"{}").unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o664)).unwrap();
        let err = inspect_file(&path).unwrap_err();
        let Error::Permissions { mode, fix, .. } = err else {
            panic!("expected Permissions, got {err:?}");
        };
        assert_eq!((mode, fix), (0o664, "600"));
        fs::set_permissions(&path, Permissions::from_mode(0o644)).unwrap();
        assert!(
            inspect_file(&path).unwrap(),
            "readable by others is allowed"
        );
    }

    #[test]
    fn a_directory_writable_by_others_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), Permissions::from_mode(0o777)).unwrap();
        let err = inspect_dir(dir.path()).unwrap_err();
        fs::set_permissions(dir.path(), Permissions::from_mode(0o700)).unwrap();
        assert!(
            matches!(err, Error::Permissions { fix: "700", .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn a_directory_owned_by_another_user_is_refused() {
        // `/` belongs to root: foreign to an ordinary user, and acceptable to
        // root itself. Either way the outcome is asserted, never skipped.
        use std::os::unix::fs::MetadataExt;
        let owner = fs::metadata("/").unwrap().uid();
        let uid = rustix::process::geteuid().as_raw();
        let result = inspect_dir(Path::new("/"));
        if owner == uid {
            assert!(result.unwrap());
        } else {
            let err = result.unwrap_err();
            assert!(
                matches!(err, Error::ForeignOwner { owner: o, uid: u, .. } if o == owner && u == uid),
                "got {err:?}"
            );
        }
    }

    #[test]
    fn a_symlinked_file_is_refused_and_a_lock_never_follows_one() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("elsewhere");
        fs::write(&target, b"precious").unwrap();
        let link = dir.path().join("data.json");
        symlink(&target, &link).unwrap();
        assert!(matches!(inspect_file(&link), Err(Error::Symlink { .. })));

        let lock = dir.path().join(".lock");
        symlink(&target, &lock).unwrap();
        let err = LockGuard::acquire(&lock, "t", Duration::from_secs(1)).unwrap_err();
        assert!(matches!(err, Error::Symlink { .. }), "got {err:?}");
        assert_eq!(fs::read(&target).unwrap(), b"precious");
    }

    #[test]
    fn append_private_creates_the_log_owner_only_and_appends_in_order() {
        let dir = crate::store::tests::private_tempdir();
        let log = dir.path().join("audit.jsonl");
        append_private(&log, b"one\n").unwrap();
        append_private(&log, b"two\n").unwrap();
        assert_eq!(fs::read(&log).unwrap(), b"one\ntwo\n");
        assert_eq!(mode(&log), 0o600);
    }

    #[test]
    fn append_private_never_follows_a_link() {
        let dir = crate::store::tests::private_tempdir();
        let target = dir.path().join("elsewhere");
        fs::write(&target, b"precious").unwrap();
        let link = dir.path().join("audit.jsonl");
        symlink(&target, &link).unwrap();
        let err = append_private(&link, b"x\n").unwrap_err();
        assert!(matches!(err, Error::Symlink { .. }), "got {err:?}");
        assert_eq!(fs::read(&target).unwrap(), b"precious");
    }
}
