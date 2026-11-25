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
    assert_eq!(add.command, "note add");
    assert!(Command::List(ListFilter::default()).audit().is_none());
    assert!(Command::Show(NoteId::FIRST).audit().is_none());
}

#[test]
fn lock_timeout_audited_command_waits_one_deadline_and_records_one_failure_line() {
    use crate::app::note::NoteApp;
    use crate::runtime::{Ctx, run_with};
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
    let store = Store::open(dir.path()).with_lock_wait(deadline);
    let clock = OffsetDateTime::now_utc;
    let ctx = Ctx::new(&store, Some("alice"), &clock);
    let cmd = Command::Add(new_note("blocked"));

    let started = Instant::now();
    let mut audit_written_before_finish = false;
    let err_code = run_with::<NoteApp, _>(&ctx, cmd, |outcome| {
        // The audit guard must not have dropped before finish runs.
        audit_written_before_finish = ctx.store().audit_path().exists();
        outcome.unwrap_err().code()
    });
    let elapsed = started.elapsed();
    drop(release_tx);
    held.join().unwrap();

    assert_eq!(err_code, "store_locked");
    assert!(
        !audit_written_before_finish,
        "finish must run before the audit guard drops"
    );
    assert!(
        elapsed >= deadline && elapsed < deadline * 2,
        "expected roughly one deadline ({deadline:?}), took {elapsed:?}"
    );

    let audit_raw = std::fs::read_to_string(ctx.store().audit_path()).unwrap();
    let lines: Vec<serde_json::Value> = audit_raw
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 1, "expected 1 audit line, got {lines:?}");
    assert_eq!(lines[0]["command"], "note add");
    assert_eq!(lines[0]["status"], "failure");
    assert_eq!(lines[0]["error_code"], "store_locked");
    assert_eq!(lines[0]["actor"], "alice");
}
