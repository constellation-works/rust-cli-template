use crate::contract::{AuditEvent, AuditStatus};
use crate::db::audit::{AUDIT_FORMAT, AuditLine, UPGRADES, decode_line, encode_line};
use crate::error::Error;
use std::path::Path;
use time::macros::datetime;

const PATH: &str = "audit.jsonl";

fn failed_add() -> AuditEvent {
    AuditEvent {
        at: datetime!(2026-09-26 12:00:00 UTC),
        actor: Some(String::from("alice")),
        command: String::from("note add"),
        status: AuditStatus::Failure,
        target: None,
        error_code: Some(String::from("store_locked")),
        duration_ms: 42,
    }
}

#[test]
fn the_audit_format_is_one_more_than_the_number_of_upgrade_steps() {
    assert_eq!(usize::try_from(AUDIT_FORMAT).unwrap(), UPGRADES.len() + 1);
}

#[test]
fn a_line_is_one_json_object_with_its_format_and_a_newline() {
    let line = encode_line(&failed_add()).unwrap();
    assert_eq!(line.last(), Some(&b'\n'));
    assert_eq!(line.iter().filter(|&&b| b == b'\n').count(), 1);
    let value: serde_json::Value = serde_json::from_slice(&line).unwrap();
    assert_eq!(value["format"], AUDIT_FORMAT);
    assert_eq!(value["actor"], "alice");
    assert_eq!(value["status"], "failure");
    assert_eq!(value["at"], "2026-09-26T12:00:00Z");
}

#[test]
fn a_line_round_trips_losslessly() {
    // STD-02 R16.
    let line = encode_line(&failed_add()).unwrap();
    let back: AuditLine = decode_line(Path::new(PATH), &line).unwrap();
    assert_eq!(back.format, AUDIT_FORMAT);
    assert_eq!(back.event, failed_add());
}

#[test]
fn an_unset_actor_serializes_as_null() {
    let mut event = failed_add();
    event.actor = None;
    let line = encode_line(&event).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&line).unwrap();
    assert!(value.get("actor").is_some_and(serde_json::Value::is_null));
}

#[test]
fn a_format_one_audit_line_without_actor_upgrades_to_null_actor() {
    let v1 = br#"{"format":1,"at":"2026-09-26T12:00:00Z","command":"note add","status":"success","target":"1","error_code":null,"duration_ms":5}"#;
    let upgraded = decode_line(Path::new(PATH), v1).unwrap();
    assert_eq!(upgraded.format, AUDIT_FORMAT);
    assert_eq!(upgraded.event.actor, None);
    assert_eq!(upgraded.event.command, "note add");
    assert_eq!(upgraded.event.status, AuditStatus::Success);
    assert_eq!(upgraded.event.target.as_deref(), Some("1"));
}

#[test]
fn a_newer_audit_line_format_is_refused() {
    let newer = format!(r#"{{"format": {}, "command": "future"}}"#, AUDIT_FORMAT + 1);
    let err = decode_line(Path::new(PATH), newer.as_bytes()).unwrap_err();
    assert!(
        matches!(err, Error::NewerFormat { found, supported, .. } if found == AUDIT_FORMAT + 1 && supported == AUDIT_FORMAT),
        "got {err:?}"
    );
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
            "actor",
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
