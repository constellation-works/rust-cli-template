use crate::store::audit::{AUDIT_FORMAT, AuditLine, encode_line};
use crate::store::{AuditEvent, AuditStatus};
use time::macros::datetime;

fn failed_add() -> AuditEvent {
    AuditEvent {
        at: datetime!(2026-09-26 12:00:00 UTC),
        command: String::from("note add"),
        status: AuditStatus::Failure,
        target: None,
        error_code: Some(String::from("store_locked")),
        duration_ms: 42,
    }
}

#[test]
fn a_line_is_one_json_object_with_its_format_and_a_newline() {
    let line = encode_line(&failed_add()).unwrap();
    assert_eq!(line.last(), Some(&b'\n'));
    assert_eq!(line.iter().filter(|&&b| b == b'\n').count(), 1);
    let value: serde_json::Value = serde_json::from_slice(&line).unwrap();
    assert_eq!(value["format"], AUDIT_FORMAT);
    assert_eq!(value["status"], "failure");
    assert_eq!(value["at"], "2026-09-26T12:00:00Z");
}

#[test]
fn a_line_round_trips_losslessly() {
    // STD-02 R16.
    let line = encode_line(&failed_add()).unwrap();
    let back: AuditLine = serde_json::from_slice(&line).unwrap();
    assert_eq!(back.format, AUDIT_FORMAT);
    assert_eq!(back.event, failed_add());
}

#[test]
fn an_event_has_no_field_for_arguments_or_error_text() {
    // STD-05 R13: the shape itself keeps note contents and paths out.
    let line = encode_line(&failed_add()).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&line).unwrap();
    let mut keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "at",
            "command",
            "duration_ms",
            "error_code",
            "format",
            "status",
            "target"
        ]
    );
}
