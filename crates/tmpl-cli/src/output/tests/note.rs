use crate::output::Payload;
use crate::output::note::NotePayload;
use crate::output::tests::support;
use cw_cli::{Format, Probe};
use time::macros::datetime;
use tmpl_cli_core::contract::Priority;

fn note(id: u64) -> NotePayload {
    NotePayload {
        id,
        title: format!("note {id}"),
        body: None,
        tags: Vec::new(),
        priority: Priority::High,
        created_at: datetime!(2026-01-02 03:04:05 UTC),
    }
}

fn notes(notes: Vec<NotePayload>, total: usize) -> Payload {
    Payload::Notes {
        notes,
        total,
        skipped: Vec::new(),
    }
}

#[test]
fn a_high_priority_is_painted_in_a_terminal_table_only() {
    // STD-01 R17, R18.
    let listed = notes(vec![note(1)], 1);
    let terminal = support::render(Probe::terminal(None), None, listed.clone());
    assert!(terminal.contains("\x1b[33mhigh\x1b[0m"), "{:?}", terminal);
    assert!(!support::piped(Format::Table, listed).contains('\x1b'));
}
