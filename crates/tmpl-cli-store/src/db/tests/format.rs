use crate::contracts::{NoteError, NoteId};
use crate::db::format::{FORMAT, StoreDocument, UPGRADES, decode, encode};
use crate::error::{BadRecordReason, Error, RecordLocator};
use std::path::Path;

const PATH: &str = "notes.json";

#[test]
fn the_format_is_one_more_than_the_number_of_upgrade_steps() {
    // STD-03 R23: a format bump without its upgrade step, or the reverse,
    // fails here rather than stranding older stores.
    assert_eq!(usize::try_from(FORMAT).unwrap(), UPGRADES.len() + 1);
}

#[test]
fn an_encoded_document_decodes_to_the_same_notes() {
    let bytes = encode(&StoreDocument::empty()).unwrap();
    let doc = decode(Path::new(PATH), &bytes).unwrap();
    assert_eq!(doc.format, FORMAT);
    assert!(doc.notes.is_empty());
    assert!(doc.bad_records.is_empty());
}

#[test]
fn a_newer_format_is_refused_by_version_not_by_shape() {
    // STD-03 R10: the unknown `shape` field must not turn this into a parse
    // error; the format number decides first.
    let bytes = format!(r#"{{"format": {}, "shape": "new"}}"#, FORMAT + 1);
    let err = decode(Path::new(PATH), bytes.as_bytes()).unwrap_err();
    assert!(
        matches!(err, Error::NewerFormat { found, supported, .. } if found == FORMAT + 1 && supported == FORMAT),
        "got {err:?}"
    );
}

#[test]
fn format_zero_and_garbage_read_as_corrupt() {
    for bytes in [
        &br#"{"format": 0}"#[..],
        b"not json",
        br#"{"notes": []}"#,
        br#"{"format": 1}"#,
    ] {
        let err = decode(Path::new(PATH), bytes).unwrap_err();
        assert!(matches!(err, Error::Corrupt { .. }), "got {err:?}");
    }
}

#[test]
fn bad_records_are_isolated_with_id_or_line_and_reason() {
    let raw = r#"{
  "format": 1,
  "notes": [
    {"id": 1, "title": "good", "body": null, "tags": ["home"], "priority": "normal", "created_at": "2026-01-02T03:04:05Z"},
    {"id": 2, "title": "   ", "body": null, "tags": [], "priority": "normal", "created_at": "2026-01-02T03:04:05Z"},
    {"id": 3, "title": "bad tag", "body": null, "tags": ["BAD"], "priority": "normal", "created_at": "2026-01-02T03:04:05Z"},
    {"id": 4, "title": "missing priority", "body": null, "tags": [], "created_at": "2026-01-02T03:04:05Z"},
    {"id": 5, "title": "missing body", "tags": [], "priority": "normal", "created_at": "2026-01-02T03:04:05Z"},
    {"title": "no id", "body": null, "tags": [], "priority": "normal", "created_at": "2026-01-02T03:04:05Z"}
  ]
}"#;
    let doc = decode(Path::new(PATH), raw.as_bytes()).unwrap();
    assert_eq!(doc.notes.len(), 1);
    assert_eq!(doc.notes[0].id.get(), 1);
    assert_eq!(doc.bad_records.len(), 5);

    assert_eq!(
        doc.bad_records[0].locator,
        RecordLocator::Id(NoteId::try_from(2).unwrap())
    );
    assert_eq!(
        doc.bad_records[0].reason,
        BadRecordReason::Validation(NoteError::EmptyTitle)
    );

    assert_eq!(
        doc.bad_records[1].locator,
        RecordLocator::Id(NoteId::try_from(3).unwrap())
    );
    assert_eq!(
        doc.bad_records[1].reason,
        BadRecordReason::Validation(NoteError::InvalidTag(String::from("BAD")))
    );

    assert_eq!(
        doc.bad_records[2].locator,
        RecordLocator::Id(NoteId::try_from(4).unwrap())
    );
    assert_eq!(
        doc.bad_records[2].reason,
        BadRecordReason::MissingField("priority")
    );

    assert_eq!(
        doc.bad_records[3].locator,
        RecordLocator::Id(NoteId::try_from(5).unwrap())
    );
    assert_eq!(
        doc.bad_records[3].reason,
        BadRecordReason::MissingField("body")
    );

    assert_eq!(doc.bad_records[4].locator, RecordLocator::Line(9));
    assert_eq!(
        doc.bad_records[4].reason,
        BadRecordReason::MissingField("id")
    );
}
