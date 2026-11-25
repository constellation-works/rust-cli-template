use crate::app::note::{Command, ListFilter, NewNote};
use time::OffsetDateTime;
use tmpl_cli_store::contract::{NoteId, Priority};

fn new_note(title: &str) -> NewNote {
    NewNote {
        title: title.parse().unwrap(),
        body: None,
        tags: Vec::new(),
        priority: Priority::Normal,
    }
}

#[test]
fn only_note_add_is_audited() {
    // STD-01 R31: list and show are read-only, so they write no audit line.
    let add = Command::Add(new_note("t")).audit().unwrap();
    assert_eq!(add.command(), "note add");
    assert!(Command::List(ListFilter::default()).audit().is_none());
    assert!(Command::Show(NoteId::FIRST).audit().is_none());
}

#[test]
fn lock_timeout_audited_command_waits_one_deadline_and_records_one_failure_line() {
    use crate::app::note::exec;
    use crate::composition::Runtime;
    use cw_app::{Origin, Surface};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};
    use tmpl_cli_store::Store;

    // The kit's private (0700) temp root, as a store directory must be.
    let dir = cw_testkit::TempRoot::new("TMPL_CLI").unwrap();

    // Another writer holds a write transaction for the duration of the
    // command, as another process would.
    let (taken_tx, taken_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let holder = Store::open(dir.path());
    let held = std::thread::spawn(move || {
        holder
            .write(|_tx| {
                taken_tx.send(()).unwrap();
                let _ = release_rx.recv();
                Ok(())
            })
            .unwrap();
    });
    taken_rx.recv().unwrap();

    let deadline = Duration::from_millis(80);
    let runtime = Runtime::new(dir.path(), Store::open(dir.path()).with_lock_wait(deadline))
        .with_clock(cw_base::Clock::fixed(OffsetDateTime::UNIX_EPOCH));
    let origin = Origin::new(Surface::CLI, Some("alice".to_owned()));
    let audit_path = runtime.audit_log().path();

    let started = Instant::now();
    let finished = exec(&runtime, &origin, Command::Add(new_note("blocked")));
    let elapsed = started.elapsed();
    let err_code = finished.result().unwrap_err().code();
    // The surface renders before it settles: no line exists yet.
    let audit_written_before_settle = audit_path.exists();
    let (_result, warning) = finished.settle();
    drop(release_tx);
    held.join().unwrap();

    assert_eq!(err_code, "store_locked");
    assert!(warning.is_none(), "unexpected audit warning: {warning:?}");
    assert!(
        !audit_written_before_settle,
        "the audit line is written by settle, after rendering"
    );
    assert!(
        elapsed >= deadline && elapsed < deadline * 2,
        "expected roughly one deadline ({deadline:?}), took {elapsed:?}"
    );

    let audit_raw = fixture::read(&audit_path);
    let lines: Vec<serde_json::Value> = audit_raw
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 1, "expected 1 audit line, got {lines:?}");
    assert_eq!(lines[0]["command"], "note add");
    assert_eq!(lines[0]["status"], "failure");
    assert_eq!(lines[0]["error_code"], "store_locked");
    assert_eq!(lines[0]["actor"], "alice");
    assert_eq!(lines[0]["surface"], "cli");
    assert_eq!(lines[0]["at"], "1970-01-01T00:00:00Z");
}

/// Files the tests inspect after a command, outside the code under test.
/// Only this module is exempt from the file bans in `clippy.toml`.
mod fixture {
    #![expect(
        clippy::disallowed_methods,
        reason = "fixture files beside the code under test; only this module is exempt"
    )]

    use std::path::Path;

    /// The text of the file `path`.
    pub(super) fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }
}
