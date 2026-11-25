use super::{piped, render};
use crate::output::{Output, Payload, StatusPayload};
use cw_cli::{Format, Probe};

fn status(format: Option<u32>) -> Payload {
    Payload::Status(StatusPayload {
        root: String::from("/data"),
        format,
    })
}

#[test]
fn status_is_a_one_row_table_a_plain_line_or_a_json_object() {
    // STD-01 R9, R11, R14.
    assert_eq!(
        piped(Format::Table, status(Some(3))),
        "ROOT   FORMAT\n/data       3\n"
    );
    assert_eq!(piped(Format::Auto, status(None)), "/data\t-\n");
    let json: serde_json::Value = serde_json::from_str(&piped(Format::Json, status(None))).unwrap();
    assert_eq!(json, serde_json::json!({"root": "/data", "format": null}));
}

#[test]
fn a_missing_format_is_muted_only_where_colour_is_on() {
    // STD-01 R17, R18: colour adds emphasis and never carries the value.
    let coloured = render(Probe::terminal(None), None, Ok(Output::new(status(None))));
    assert!(
        coloured.out.contains("\x1b[90m-\x1b[0m"),
        "{:?}",
        coloured.out
    );
    assert!(!piped(Format::Table, status(None)).contains('\x1b'));
}
