//! End-to-end tests of the note app against a real store (STD-02 R20), run
//! through the kit's chokepoint with the clock pinned by
//! `Runtime::with_clock(Clock::fixed(..))`.
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
#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "a test sets up fixture files and runs the built binary; production code may not"
)]

use cw_base::Clock;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Arc;
use time::OffsetDateTime;
use time::macros::datetime;
use tmpl_cli_core::app::note::{self, Command, ListFilter, NewNote, NoteList, Outcome};
use tmpl_cli_core::contract::{Note, NoteId, Priority, Tag};
use tmpl_cli_core::{Error, Origin, Runtime, Surface};
use tmpl_cli_store::contract::NoteError;
use tmpl_cli_store::{
    BadRecord, BadRecordReason, Error as StoreError, RecordLocator, STORE_FILE, Store,
};

const NOW: OffsetDateTime = datetime!(2026-01-02 03:04:05 UTC);

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
        let Outcome::Added { note, .. } = run(self, now, Command::Add(new))? else {
            panic!("add returns the added note");
        };
        Ok(note)
    }

    fn list(&self, filter: &ListFilter) -> Result<Loaded<NoteList>, Error> {
        let Outcome::Listed {
            list, bad_records, ..
        } = run(self, NOW, Command::List(filter.clone()))?
        else {
            panic!("list returns the listing");
        };
        Ok(Loaded {
            value: list,
            bad_records,
        })
    }

    fn show(&self, id: NoteId) -> Result<Loaded<Note>, Error> {
        let Outcome::Shown { note, bad_records } = run(self, NOW, Command::Show(id))? else {
            panic!("show returns the note");
        };
        Ok(Loaded {
            value: note,
            bad_records,
        })
    }
}

/// The store failure behind an error, when there is one.
fn store_error(err: &Error) -> Option<&StoreError> {
    err.downcast_ref::<StoreError>()
}

/// Run `command` through the chokepoint against `store`, its data directory
/// the store's, with the clock pinned at `now`; the audit record is settled
/// after the result is taken, as a surface does.
fn run(store: &Store, now: OffsetDateTime, command: Command) -> Result<Outcome, Error> {
    let root = store.path().parent().unwrap().to_path_buf();
    let runtime = Runtime::new(root, store.clone()).with_clock(Clock::fixed(now));
    let origin = Origin::new(Surface::CLI, None);
    let (result, _warning) = note::exec(&runtime, &origin, command).settle();
    result
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
    assert!(matches!(
        err.downcast_ref::<note::Error>(),
        Some(note::Error::NotFound { .. })
    ));
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

/// The ids of a listing's rows, in the order the listing returns them.
fn ids(list: &NoteList) -> Vec<u64> {
    list.notes.iter().map(|note| note.id.get()).collect()
}

#[test]
fn filters_combine_and_keep_the_stored_order() {
    // Both filters hold at once, and the matches keep the stored order (the
    // ascending id), not the order of their titles.
    let dir = tempdir();
    let store = Store::open(dir.path());
    store
        .add(new_note("zeta", &["home"], Priority::High), NOW)
        .unwrap();
    store
        .add(new_note("beta", &["work"], Priority::High), NOW)
        .unwrap();
    store
        .add(new_note("mid", &["home"], Priority::Low), NOW)
        .unwrap();
    store
        .add(new_note("alpha", &["home"], Priority::High), NOW)
        .unwrap();

    let listed = store
        .list(&ListFilter {
            tag: Some("home".parse().unwrap()),
            priority: Some(Priority::High),
            limit: None,
        })
        .unwrap()
        .value;
    assert_eq!(ids(&listed), [1, 4]);
    assert_eq!(listed.total, 2);
    assert!(!listed.truncated());
}

#[test]
fn the_limit_applies_after_the_filters_and_the_total_counts_every_match() {
    // STD-01 R33, R34: the filter narrows the rows before the limit cuts
    // them, and the total and the truncation count every filtered match.
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
    store
        .add(new_note("d", &["home"], Priority::High), NOW)
        .unwrap();

    let listed = store
        .list(&ListFilter {
            tag: Some("home".parse().unwrap()),
            priority: None,
            limit: limit(2),
        })
        .unwrap()
        .value;
    assert_eq!(ids(&listed), [1, 3]);
    assert_eq!(listed.total, 3);
    assert!(listed.truncated());
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
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !handle.is_finished() {
            assert!(
                std::time::Instant::now() < deadline,
                "writer thread timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
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
fn a_hand_edited_value_is_escaped_in_its_bad_record_reason() {
    // A record edited by hand can hold control bytes. The reason a read
    // reports, and the warning a surface prints from it, escapes them.
    let dir = tempdir();
    let store = Store::open(dir.path());
    execute(
        &store,
        "INSERT INTO note (id, title, body, tags, priority, created_at) VALUES
            (1, 'bad', NULL, '[]', 'hi' || char(27) || char(10) || char(9), '2025-05-06T07:08:09Z');",
    );

    let listed = store.list(&ListFilter::default()).unwrap();
    assert_eq!(listed.bad_records.len(), 1);
    assert_eq!(
        listed.bad_records[0].reason.to_string(),
        "invalid priority \"hi\\u{1b}\\n\\t\": expected one of low, normal, high"
    );
}

#[test]
fn the_next_id_is_one_past_the_highest_whatever_the_stored_order() {
    let dir = tempdir();
    let store = Store::open(dir.path());
    execute(
        &store,
        "INSERT INTO note (id, title, body, tags, priority, created_at) VALUES
            (3, 'three', NULL, '[]', 'normal', '2026-01-02T03:04:05Z'),
            (1, 'one', NULL, '[]', 'normal', '2026-01-02T03:04:05Z'),
            (7, 'seven', NULL, '[]', 'normal', '2026-01-02T03:04:05Z');",
    );
    let added = store
        .add(new_note("next", &[], Priority::Normal), NOW)
        .unwrap();
    assert_eq!(added.id.get(), 8);
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

#[test]
fn an_add_is_audited_and_its_line_reads_back_through_the_kits_reader() {
    // STD-01 R31: a mutating command leaves one audit line, in the kit's
    // format, that the kit's own reader parses back.
    let dir = tempdir();
    let store = Store::open(dir.path());
    let note = store
        .add(new_note("audited", &[], Priority::Normal), NOW)
        .unwrap();
    store.list(&ListFilter::default()).unwrap();
    let lines = cw_app::AuditLog::new(store.root())
        .read(cw_fs::Cap::bytes(1 << 20))
        .unwrap();
    let [Ok(record)] = lines.as_slice() else {
        panic!("one audit line, for the add alone: {lines:?}");
    };
    assert_eq!(record.command, "note add");
    assert_eq!(record.status, cw_app::Status::Success);
    assert_eq!(record.surface.as_deref(), Some("cli"));
    assert_eq!(record.target, Some(note.id.to_string()));
    assert_eq!(record.error_code, None);
}
