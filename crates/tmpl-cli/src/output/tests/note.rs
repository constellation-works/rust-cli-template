use crate::output::Payload;
use crate::output::note::{ListedNotePayload, NotePayload, NoteSummaryPayload};
use crate::output::tests::support;
use cw_cli::{Format, Probe};
use time::macros::datetime;
use tmpl_cli_core::contract::Priority;

fn summary(id: u64) -> ListedNotePayload {
    ListedNotePayload::Summary(NoteSummaryPayload {
        id,
        title: format!("note {id}"),
        tags: Vec::new(),
        priority: Priority::High,
        created_at: datetime!(2026-01-02 03:04:05 UTC),
    })
}

fn full(id: u64, body: Option<&str>) -> ListedNotePayload {
    ListedNotePayload::Full(NotePayload {
        id,
        title: format!("note {id}"),
        body: body.map(str::to_owned),
        tags: Vec::new(),
        priority: Priority::Normal,
        created_at: datetime!(2026-01-02 03:04:05 UTC),
    })
}

fn notes(notes: Vec<ListedNotePayload>, total: usize, include_body: bool) -> Payload {
    Payload::Notes {
        notes,
        total,
        include_body,
        skipped: Vec::new(),
    }
}

#[test]
fn a_high_priority_is_painted_in_a_terminal_table_only() {
    // STD-01 R17, R18.
    let listed = notes(vec![summary(1)], 1, false);
    let terminal = support::render(Probe::terminal(None), None, listed.clone());
    assert!(terminal.contains("\x1b[33mhigh\x1b[0m"), "{:?}", terminal);
    assert!(!support::piped(Format::Table, listed).contains('\x1b'));
}

#[test]
fn compact_listing_omits_body_and_include_body_adds_it() {
    let compact_json = support::piped(Format::Json, notes(vec![summary(1)], 1, false));
    let parsed: serde_json::Value = serde_json::from_str(&compact_json).unwrap();
    assert!(parsed["notes"][0].get("body").is_none(), "{compact_json}");

    let compact_table = support::piped(Format::Table, notes(vec![summary(1)], 1, false));
    assert!(!compact_table.contains("BODY"), "{compact_table}");

    let with_body = notes(vec![full(1, Some("hello\nworld")), full(2, None)], 2, true);
    let full_json = support::piped(Format::Json, with_body.clone());
    let parsed: serde_json::Value = serde_json::from_str(&full_json).unwrap();
    assert_eq!(parsed["notes"][0]["body"], "hello\nworld");
    assert_eq!(parsed["notes"][1]["body"], serde_json::Value::Null);

    let full_table = support::piped(Format::Table, with_body);
    assert!(full_table.contains("BODY"), "{full_table}");
    assert!(full_table.contains("hello\\nworld"), "{full_table}");
}
