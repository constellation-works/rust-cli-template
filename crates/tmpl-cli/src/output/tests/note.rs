use super::{piped, render};
use crate::output::note::{NotePayload, detail_text};
use crate::output::{Output, Payload};
use cw_cli::{Format, Probe};
use std::process::ExitCode;
use time::macros::datetime;
use tmpl_cli_core::Error;
use tmpl_cli_core::contract::Priority;

fn note(id: u64, tags: &[&str]) -> NotePayload {
    NotePayload {
        id,
        title: format!("note {id}"),
        body: None,
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        priority: Priority::High,
        created_at: datetime!(2026-01-02 03:04:05 UTC),
    }
}

fn notes(notes: Vec<NotePayload>, total: usize) -> Payload {
    Payload::Notes { notes, total }
}

#[test]
fn the_plain_form_is_one_tab_separated_line_per_record() {
    // STD-01 R9.
    assert_eq!(
        piped(Format::Auto, Payload::Note(note(3, &["a", "b"]))),
        "3\thigh\tnote 3\ta,b\t2026-01-02T03:04:05Z\n"
    );
    let text = piped(Format::Auto, notes(vec![note(1, &[]), note(2, &[])], 2));
    assert_eq!(text.lines().count(), 2);
    assert!(text.lines().all(|line| line.split('\t').count() == 5));
}

#[test]
fn an_empty_list_is_empty_stdout_in_human_modes_and_an_empty_envelope_in_json() {
    // STD-01 R16 (and R7: JSON mode always emits one document).
    assert_eq!(piped(Format::Auto, notes(Vec::new(), 0)), "");
    assert_eq!(piped(Format::Table, notes(Vec::new(), 0)), "");
    let json: serde_json::Value =
        serde_json::from_str(&piped(Format::Json, notes(Vec::new(), 0))).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"notes": [], "total": 0, "truncated": false})
    );
}

#[test]
fn a_cut_list_says_so_in_the_json_envelope() {
    // STD-01 R34.
    let json: serde_json::Value =
        serde_json::from_str(&piped(Format::Json, notes(vec![note(1, &[])], 4))).unwrap();
    assert_eq!(json["total"], 4);
    assert_eq!(json["truncated"], true);
    assert_eq!(json["notes"].as_array().map(Vec::len), Some(1));
}

#[test]
fn json_carries_null_for_absent_values_rather_than_omitting_them() {
    // STD-01 R11.
    let value: serde_json::Value =
        serde_json::from_str(&piped(Format::Json, Payload::Note(note(1, &[])))).unwrap();
    assert!(value.get("body").is_some_and(serde_json::Value::is_null));
    assert_eq!(value["tags"], serde_json::json!([]));
    assert_eq!(value["priority"], "high");
    assert_eq!(value["created_at"], "2026-01-02T03:04:05Z");
}

#[test]
fn a_single_record_list_is_still_a_table() {
    // STD-01 R16: the shape does not depend on the record count.
    let text = piped(Format::Table, notes(vec![note(1, &[])], 1));
    assert_eq!(text.lines().count(), 2);
    assert!(text.starts_with("ID"));
}

#[test]
fn a_high_priority_is_painted_in_a_terminal_table_only() {
    // STD-01 R17, R18.
    let listed = notes(vec![note(1, &[])], 1);
    let terminal = render(Probe::terminal(None), None, Ok(Output::new(listed.clone())));
    assert!(
        terminal.out.contains("\x1b[33mhigh\x1b[0m"),
        "{:?}",
        terminal.out
    );
    assert!(!piped(Format::Table, listed).contains('\x1b'));
}

#[test]
fn detail_view_escapes_body_control_characters_and_normalizes_crlf() {
    let mut payload = note(1, &[]);
    payload.body = Some(String::from(
        "first\r\nsecond\rthird\x1b[31mred\x1b[0m\tkept",
    ));
    assert!(detail_text(&payload).contains("first\n          second\rthird"));
    let rendered = piped(Format::Table, Payload::Note(payload));
    assert!(
        rendered
            .bytes()
            .all(|byte| byte >= 0x20 || byte == b'\n' || byte == b'\t'),
        "raw control byte leaked into detail output: {rendered:?}"
    );
    assert!(rendered.contains("first\n          second\\rthird\\u{1b}[31mred\\u{1b}[0m\tkept"));
}

#[test]
fn warnings_and_the_notice_go_to_stderr_before_the_payload() {
    // STD-01 R12: stdout carries the payload alone.
    let output = Output {
        payload: notes(vec![note(1, &[])], 1),
        notice: Some(String::from("1 note")),
        warnings: vec![String::from("skipped a bad record")],
    };
    let written = render(Probe::piped(), None, Ok(output));
    assert_eq!(written.err, "warning: skipped a bad record\n1 note\n");
    assert_eq!(written.out.lines().count(), 1);
}

#[derive(Debug, thiserror::Error)]
#[error("note 9 not found\nerror: forged")]
struct Missing;

#[test]
fn a_missing_note_is_reported_with_its_hint() {
    // STD-01 R19, R20, R21: the kit writes it and picks the code.
    let err = Error::new(tmpl_cli_core::Kind::NotFound, "note_not_found", Missing);
    let text = render(Probe::piped(), None, Err(err));
    assert_eq!(text.code, ExitCode::from(1));
    assert_eq!(text.out, "");
    assert_eq!(
        text.err,
        "error: note 9 not found\\nerror: forged\n\
         hint: run 'tmpl-cli note list' with the same --root to see existing ids\n"
    );
    let err = Error::new(tmpl_cli_core::Kind::NotFound, "note_not_found", Missing);
    let json = render(Probe::piped(), Some(Format::Json), Err(err));
    let value: serde_json::Value = serde_json::from_str(&json.err).unwrap();
    assert_eq!(value["code"], "note_not_found");
    assert!(
        value["hint"]
            .as_str()
            .is_some_and(|hint| hint.contains("note list"))
    );
}
