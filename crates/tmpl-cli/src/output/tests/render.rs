use crate::output::payload::NotePayload;
use crate::output::render::{note_detail, note_list, plain_line};
use crate::output::sink::{OutputMode, Sink};
use time::macros::datetime;
use tmpl_cli_core::Priority;

fn note(id: u64, tags: &[&str]) -> NotePayload {
    NotePayload {
        id,
        title: format!("note {id}"),
        body: None,
        tags: tags.iter().map(|t| (*t).to_owned()).collect(),
        priority: Priority::High,
        created_at: datetime!(2026-01-02 03:04:05 UTC),
    }
}

fn sink(mode: OutputMode) -> Sink {
    Sink {
        mode,
        color: false,
        width: None,
    }
}

fn render_list(mode: OutputMode, notes: &[NotePayload]) -> String {
    let mut out = Vec::new();
    note_list(&mut out, &sink(mode), notes, notes.len()).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn the_plain_form_is_one_tab_separated_line_per_record() {
    // STD-01 R9.
    assert_eq!(
        plain_line(&note(3, &["a", "b"])),
        "3\thigh\tnote 3\ta,b\t2026-01-02T03:04:05Z"
    );
    let text = render_list(OutputMode::Plain, &[note(1, &[]), note(2, &[])]);
    assert_eq!(text.lines().count(), 2);
    assert!(text.lines().all(|l| l.split('\t').count() == 5));
}

#[test]
fn an_empty_list_is_empty_stdout_in_human_modes_and_an_empty_envelope_in_json() {
    // STD-01 R16 (and R7: JSON mode always emits one document).
    assert_eq!(render_list(OutputMode::Plain, &[]), "");
    assert_eq!(render_list(OutputMode::Table, &[]), "");
    let json: serde_json::Value =
        serde_json::from_str(&render_list(OutputMode::Json, &[])).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"notes": [], "total": 0, "truncated": false})
    );
}

#[test]
fn a_cut_list_says_so_in_the_json_envelope() {
    // STD-01 R34.
    let mut out = Vec::new();
    note_list(&mut out, &sink(OutputMode::Json), &[note(1, &[])], 4).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(json["total"], 4);
    assert_eq!(json["truncated"], true);
    assert_eq!(json["notes"].as_array().map(Vec::len), Some(1));
}

#[test]
fn json_carries_null_for_absent_values_rather_than_omitting_them() {
    // STD-01 R11.
    let mut out = Vec::new();
    note_detail(&mut out, &sink(OutputMode::Json), &note(1, &[])).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert!(value.get("body").is_some_and(serde_json::Value::is_null));
    assert_eq!(value["tags"], serde_json::json!([]));
    assert_eq!(value["priority"], "high");
    assert_eq!(value["created_at"], "2026-01-02T03:04:05Z");
}

#[test]
fn a_single_record_list_is_still_a_table() {
    // STD-01 R16: the shape does not depend on the record count.
    let text = render_list(OutputMode::Table, &[note(1, &[])]);
    assert_eq!(text.lines().count(), 2);
    assert!(text.starts_with("ID"));
}
