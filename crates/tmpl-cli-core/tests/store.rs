//! End-to-end tests of the public store API (STD-02 R20). Every test works in
//! its own temporary directory and never touches the real home (STD-03 R20).
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Arc;
use time::OffsetDateTime;
use time::macros::datetime;
use tmpl_cli_core::{Error, ListFilter, NewNote, NoteId, Priority, STORE_FILE, Store, Tag};

const NOW: OffsetDateTime = datetime!(2026-01-02 03:04:05 UTC);

/// A temporary directory that is owner-only whatever the umask, as a store
/// directory must be (STD-05 R9). `tempfile` leaves the mode to the umask.
fn tempdir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    dir
}

/// Write a store file by hand, owner-only as the store itself would.
fn write_store_file(dir: &Path, text: &str) {
    let path = dir.join(STORE_FILE);
    std::fs::write(&path, text).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}

fn limit(n: usize) -> Option<NonZeroUsize> {
    NonZeroUsize::new(n)
}

fn new_note(title: &str, tags: &[&str], priority: Priority) -> NewNote {
    NewNote {
        title: title.parse().unwrap(),
        body: None,
        tags: tags.iter().map(|t| t.parse().unwrap()).collect(),
        priority,
    }
}

#[test]
fn a_missing_store_lists_nothing_and_creates_nothing() {
    let dir = tempdir();
    let root = dir.path().join("data");
    let store = Store::open(&root);
    let listed = store.list(&ListFilter::default()).unwrap();
    assert!(listed.notes.is_empty());
    assert_eq!(listed.total, 0);
    assert!(!root.exists(), "reads must not create the data directory");
}

#[test]
fn add_assigns_sequential_ids_and_show_finds_them() {
    let dir = tempdir();
    let store = Store::open(dir.path().join("data"));
    let first = store
        .add(new_note("one", &[], Priority::Normal), NOW)
        .unwrap();
    let second = store
        .add(new_note("two", &[], Priority::High), NOW)
        .unwrap();
    assert_eq!(first.id.get(), 1);
    assert_eq!(second.id.get(), 2);
    assert_eq!(store.show(second.id).unwrap(), second);
    assert_eq!(store.show(first.id).unwrap().created_at, NOW);
}

#[test]
fn show_of_an_unknown_id_is_not_found() {
    let dir = tempdir();
    let store = Store::open(dir.path());
    let id: NoteId = "42".parse().unwrap();
    let err = store.show(id).unwrap_err();
    assert!(matches!(err, Error::NotFound { .. }));
    assert_eq!(err.code(), "note_not_found");
    // STD-02 R26: the message names the store that was searched.
    assert!(
        err.to_string()
            .contains(&store.path().display().to_string()),
        "got {err}"
    );
}

#[test]
fn list_filters_before_the_limit_and_reports_the_total() {
    let dir = tempdir();
    let store = Store::open(dir.path());
    store
        .add(new_note("a", &["home"], Priority::High), NOW)
        .unwrap();
    store
        .add(new_note("b", &["work"], Priority::High), NOW)
        .unwrap();
    store
        .add(new_note("c", &["home"], Priority::Low), NOW)
        .unwrap();

    let home: Tag = "home".parse().unwrap();
    let titles = |filter: ListFilter| -> Vec<String> {
        store
            .list(&filter)
            .unwrap()
            .notes
            .into_iter()
            .map(|n| n.title.as_str().to_owned())
            .collect()
    };
    assert_eq!(
        titles(ListFilter {
            tag: Some(home.clone()),
            ..ListFilter::default()
        }),
        ["a", "c"]
    );
    assert_eq!(
        titles(ListFilter {
            tag: Some(home),
            priority: Some(Priority::High),
            limit: None,
        }),
        ["a"]
    );
    assert_eq!(
        titles(ListFilter {
            limit: limit(2),
            ..ListFilter::default()
        }),
        ["a", "b"]
    );
    // STD-01 R33: the one `work` note is not among the first record, and the
    // limit still finds it because filters apply first.
    assert_eq!(
        titles(ListFilter {
            tag: Some("work".parse().unwrap()),
            limit: limit(1),
            ..ListFilter::default()
        }),
        ["b"]
    );
    // STD-01 R34: a cut list knows how many matched.
    let cut = store
        .list(&ListFilter {
            limit: limit(2),
            ..ListFilter::default()
        })
        .unwrap();
    assert_eq!((cut.notes.len(), cut.total, cut.truncated()), (2, 3, true));
    let whole = store.list(&ListFilter::default()).unwrap();
    assert_eq!(
        (whole.notes.len(), whole.total, whole.truncated()),
        (3, 3, false)
    );
}

#[test]
fn concurrent_writers_never_lose_a_note_or_reuse_an_id() {
    let dir = tempdir();
    let store = Arc::new(Store::open(dir.path()));
    let threads: Vec<_> = (0..4)
        .map(|t| {
            let store = Arc::clone(&store);
            std::thread::spawn(move || {
                for i in 0..5 {
                    store
                        .add(new_note(&format!("t{t}-{i}"), &[], Priority::Normal), NOW)
                        .unwrap();
                }
            })
        })
        .collect();
    for handle in threads {
        handle.join().unwrap();
    }
    let mut ids: Vec<u64> = store
        .list(&ListFilter::default())
        .unwrap()
        .notes
        .iter()
        .map(|n| n.id.get())
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, (1..=20).collect::<Vec<_>>());
}

#[test]
fn a_store_from_a_newer_build_is_refused_not_rewritten() {
    let dir = tempdir();
    let path = dir.path().join(STORE_FILE);
    let newer = r#"{"format": 2, "records": {"shape": "unknown to this build"}}"#;
    write_store_file(dir.path(), newer);
    let store = Store::open(dir.path());

    let err = store.list(&ListFilter::default()).unwrap_err();
    assert!(
        matches!(
            err,
            Error::NewerFormat {
                found: 2,
                supported: 1,
                ..
            }
        ),
        "got {err:?}"
    );
    let err = store
        .add(new_note("x", &[], Priority::Normal), NOW)
        .unwrap_err();
    assert_eq!(err.code(), "store_too_new");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), newer);
    // STD-02 R34: refused before the first side effect, so no lock file either.
    assert!(!dir.path().join(".lock").exists());
}

#[test]
fn a_corrupt_store_is_reported_with_its_path() {
    let dir = tempdir();
    write_store_file(dir.path(), "not json");
    let err = Store::open(dir.path())
        .list(&ListFilter::default())
        .unwrap_err();
    assert_eq!(err.code(), "store_corrupt");
    assert!(err.to_string().contains(STORE_FILE), "got {err}");
}

#[test]
fn a_format_one_document_with_only_required_fields_still_loads() {
    // Persisted shape contract (STD-02 R16): fields added later default.
    let dir = tempdir();
    let minimal = r#"{"format": 1, "notes": [
        {"id": 1, "title": "old", "created_at": "2025-05-06T07:08:09Z"}
    ]}"#;
    write_store_file(dir.path(), minimal);
    let notes = Store::open(dir.path())
        .list(&ListFilter::default())
        .unwrap()
        .notes;
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].priority, Priority::Normal);
    assert!(notes[0].tags.is_empty());
    assert_eq!(notes[0].body, None);
}

#[test]
fn format_zero_never_existed_and_reads_as_corrupt() {
    let dir = tempdir();
    write_store_file(dir.path(), r#"{"format": 0, "notes": []}"#);
    let err = Store::open(dir.path())
        .list(&ListFilter::default())
        .unwrap_err();
    assert_eq!(err.code(), "store_corrupt");
}
