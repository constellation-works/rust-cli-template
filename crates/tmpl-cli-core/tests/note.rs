//! End-to-end tests of the note app against a real store (STD-02 R20), run
//! through `test-support` so the clock and the lock wait can be pinned.
//! Every test works in its own temporary directory and never touches the
//! real home (STD-03 R20).
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "integration tests assert fixture setup"
    )
)]

use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Arc;
use time::OffsetDateTime;
use time::macros::datetime;
use tmpl_cli_core::app::note::{Command, Error, ListFilter, NewNote, NoteList, Outcome};
use tmpl_cli_core::contract::{Note, NoteId, Priority, Tag};
use tmpl_cli_core::test_support::{Ctx, note_run};
use tmpl_cli_store::contract::NoteError;
use tmpl_cli_store::{
    BadRecord, BadRecordReason, Error as StoreError, RecordLocator, STORE_FILE, Store,
};

const NOW: OffsetDateTime = datetime!(2026-01-02 03:04:05 UTC);

/// The store failure behind an app error, when there is one.
fn store_error(err: &Error) -> Option<&StoreError> {
    if let Error::Store(source) = err {
        source.downcast_ref::<StoreError>()
    } else {
        None
    }
}

/// A read's value together with the bad records it isolated.
#[derive(Debug)]
struct Loaded<T> {
    value: T,
    bad_records: Vec<BadRecord>,
}

/// The note app's three commands run against one store, so each test reads
/// as a sequence of operations on that store. `add` takes the creation time.
trait NoteApp {
    fn add(&self, new: NewNote, now: OffsetDateTime) -> Result<Note, Error>;
    fn list(&self, filter: &ListFilter) -> Result<Loaded<NoteList>, Error>;
    fn show(&self, id: NoteId) -> Result<Loaded<Note>, Error>;
}

impl NoteApp for Store {
    fn add(&self, new: NewNote, now: OffsetDateTime) -> Result<Note, Error> {
        let clock = move || now;
        let Outcome::Added { note, .. } =
            note_run(&Ctx::new(self, None, &clock), Command::Add(new))?
        else {
            panic!("add returns the added note");
        };
        Ok(note)
    }

    fn list(&self, filter: &ListFilter) -> Result<Loaded<NoteList>, Error> {
        let clock = || NOW;
        let command = Command::List(filter.clone());
        let Outcome::Listed {
            list, bad_records, ..
        } = note_run(&Ctx::new(self, None, &clock), command)?
        else {
            panic!("list returns the listing");
        };
        Ok(Loaded {
            value: list,
            bad_records,
        })
    }

    fn show(&self, id: NoteId) -> Result<Loaded<Note>, Error> {
        let clock = || NOW;
        let Outcome::Shown { note, bad_records } =
            note_run(&Ctx::new(self, None, &clock), Command::Show(id))?
        else {
            panic!("show returns the note");
        };
        Ok(Loaded {
            value: note,
            bad_records,
        })
    }
}

/// A temporary directory that is owner-only whatever the umask, as a store
/// directory must be (STD-05 R9). The kit's temp root is `0700`.
fn tempdir() -> cw_testkit::TempRoot {
    cw_testkit::TempRoot::new("TMPL_CLI").unwrap()
}

/// Run raw SQL on the store's database, for state the public API never
/// writes.
fn execute(store: &Store, sql: &str) {
    tmpl_cli_store::test_support::execute(store, sql).unwrap();
}

/// Write bytes at the store's database path by hand, owner-only as the
/// store itself would.
fn write_store_file(dir: &Path, text: &str) {
    let path = dir.join(STORE_FILE);
    std::fs::write(&path, text).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}

fn limit(count: usize) -> Option<NonZeroUsize> {
    NonZeroUsize::new(count)
}

fn new_note(title: &str, tags: &[&str], priority: Priority) -> NewNote {
    NewNote {
        title: title.parse().unwrap(),
        body: None,
        tags: tags.iter().map(|tag| tag.parse().unwrap()).collect(),
        priority,
    }
}

#[test]
fn a_missing_store_lists_nothing_and_creates_nothing() {
    let dir = tempdir();
    let root = dir.path().join("data");
    let store = Store::open(&root);
    let listed = store.list(&ListFilter::default()).unwrap();
    assert!(listed.value.notes.is_empty());
    assert_eq!(listed.value.total, 0);
    assert!(listed.bad_records.is_empty());
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
    assert_eq!(store.show(second.id).unwrap().value, second);
    assert_eq!(store.show(first.id).unwrap().value.created_at, NOW);
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
            .value
            .notes
            .into_iter()
            .map(|note| note.title.as_str().to_owned())
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
        .unwrap()
        .value;
    assert_eq!((cut.notes.len(), cut.total, cut.truncated()), (2, 3, true));
    let whole = store.list(&ListFilter::default()).unwrap().value;
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
        .map(|thread| {
            let store = Arc::clone(&store);
            std::thread::spawn(move || {
                for seq in 0..5 {
                    store
                        .add(
                            new_note(&format!("t{thread}-{seq}"), &[], Priority::Normal),
                            NOW,
                        )
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
        .value
        .notes
        .iter()
        .map(|note| note.id.get())
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, (1..=20).collect::<Vec<_>>());
}

#[test]
fn a_store_from_a_newer_build_is_refused_not_rewritten() {
    let dir = tempdir();
    let store = Store::open(dir.path());
    // A database a build with a second schema step migrated.
    execute(&store, "PRAGMA user_version = 2;");

    let err = store.list(&ListFilter::default()).unwrap_err();
    assert_eq!(err.code(), "store_too_new", "got {err:?}");
    let err = store
        .add(new_note("x", &[], Priority::Normal), NOW)
        .unwrap_err();
    assert_eq!(err.code(), "store_too_new", "got {err:?}");
    // Not migrated back or rewritten: a read still meets the newer version.
    let err = store.list(&ListFilter::default()).unwrap_err();
    assert_eq!(err.code(), "store_too_new", "got {err:?}");
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
fn bad_records_are_isolated_on_read_fail_on_show_and_refuse_writes() {
    let dir = tempdir();
    let store = Store::open(dir.path());
    execute(
        &store,
        "INSERT INTO note (id, title, body, tags, priority, created_at) VALUES
            (1, 'good', NULL, '[]', 'normal', '2025-05-06T07:08:09Z'),
            (2, 'bad' || char(27) || 'title', NULL, '[]', 'normal', '2025-05-06T07:08:09Z'),
            (3, 'unknown priority', NULL, '[]', 'urgent', '2025-05-06T07:08:09Z');",
    );

    let listed = store.list(&ListFilter::default()).unwrap();
    assert_eq!(listed.value.notes.len(), 1);
    assert_eq!(listed.value.notes[0].id.get(), 1);
    assert_eq!(listed.bad_records.len(), 2);
    assert_eq!(
        listed.bad_records[0].locator,
        RecordLocator::Id(NoteId::try_from(2).unwrap())
    );
    assert_eq!(
        listed.bad_records[1].locator,
        RecordLocator::Id(NoteId::try_from(3).unwrap())
    );
    assert_eq!(
        listed.bad_records[1].reason,
        BadRecordReason::Validation(NoteError::InvalidPriority(String::from("urgent")))
    );

    // Showing the good id succeeds and still surfaces the bad records.
    let shown = store.show(NoteId::try_from(1).unwrap()).unwrap();
    assert_eq!(shown.value.title.as_str(), "good");
    assert_eq!(shown.bad_records.len(), 2);

    // Showing a bad id fails with that record's typed error.
    let err = store.show(NoteId::try_from(2).unwrap()).unwrap_err();
    assert_eq!(err.code(), "store_bad_record");
    assert!(matches!(
        store_error(&err),
        Some(StoreError::BadRecord(record))
            if record.locator == RecordLocator::Id(NoteId::try_from(2).unwrap())
    ));

    // Adding refuses and adds nothing.
    let err = store
        .add(new_note("new", &[], Priority::Normal), NOW)
        .unwrap_err();
    assert_eq!(err.code(), "store_bad_record");
    let after = store.list(&ListFilter::default()).unwrap();
    assert_eq!((after.value.notes.len(), after.bad_records.len()), (1, 2));
}

#[test]
fn add_refuses_when_max_id_is_already_in_use() {
    let dir = tempdir();
    let store = Store::open(dir.path());
    execute(
        &store,
        &format!(
            "INSERT INTO note (id, title, body, tags, priority, created_at)
             VALUES ({}, 'last', NULL, '[]', 'normal', '2026-01-02T03:04:05Z');",
            NoteId::MAX
        ),
    );
    let err = store
        .add(new_note("overflow", &[], Priority::Normal), NOW)
        .unwrap_err();
    assert_eq!(err.code(), "note_id_exhausted", "got {err:?}");
}

#[test]
fn encode_failure_is_not_io_and_names_no_path() {
    let dir = tempdir();
    let store = Store::open(dir.path());
    // RFC 3339 cannot format negative years, forcing a real encode error.
    let bad_date = time::Date::from_calendar_date(-1, time::Month::January, 1).unwrap();
    let bad_time = OffsetDateTime::new_utc(bad_date, time::Time::MIDNIGHT);
    let err = store
        .add(new_note("encode fail", &[], Priority::Normal), bad_time)
        .unwrap_err();
    assert!(
        matches!(store_error(&err), Some(StoreError::Timestamp { .. })),
        "got {err:?}"
    );
    assert_eq!(err.code(), "encode_error");
    let msg = err.to_string();
    assert!(
        !msg.contains(&dir.path().display().to_string()) && !msg.contains(STORE_FILE),
        "encode error must not invent a path: {msg}"
    );
}
