use super::{piped, render};
use crate::output::{Payload, StatusPayload};
use cw_cli::{Format, Probe};

fn status(format: Option<u32>) -> Payload {
    Payload::Status(StatusPayload {
        root: String::from("/data"),
        format,
    })
}

#[test]
fn a_missing_format_is_muted_only_where_colour_is_on() {
    // STD-01 R17, R18: colour adds emphasis and never carries the value.
    let coloured = render(Probe::terminal(None), None, status(None));
    assert!(coloured.contains("\x1b[90m-\x1b[0m"), "{:?}", coloured);
    assert!(!piped(Format::Table, status(None)).contains('\x1b'));
}
