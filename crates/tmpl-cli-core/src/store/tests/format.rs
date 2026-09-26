use crate::error::Error;
use crate::store::format::{FORMAT, StoreDocument, UPGRADES, decode, encode};
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
    let bytes = encode(Path::new(PATH), StoreDocument::empty()).unwrap();
    let doc = decode(Path::new(PATH), &bytes).unwrap();
    assert_eq!(doc.format, FORMAT);
    assert!(doc.notes.is_empty());
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
    for bytes in [&br#"{"format": 0}"#[..], b"not json", br#"{"notes": []}"#] {
        let err = decode(Path::new(PATH), bytes).unwrap_err();
        assert!(matches!(err, Error::Corrupt { .. }), "got {err:?}");
    }
}
