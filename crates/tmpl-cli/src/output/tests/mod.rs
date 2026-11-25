//! Unit tests for the output layer, one file per source file (STD-02 R19).

// example:begin
mod note;

// example:end
mod status;

use crate::output::{Output, Payload, finish};
use cw_cli::{Format, Probe, Sink, Terminal};
use std::process::ExitCode;
use tmpl_cli_core::{APP, Env, Error};

/// What one command's outcome wrote: the exit code, stdout and stderr.
pub(super) struct Written {
    pub(super) code: ExitCode,
    pub(super) out: String,
    pub(super) err: String,
}

/// Render `outcome` through the kit's sink, as a command does, for a reader
/// `probe` describes asking for `format`.
pub(super) fn render(
    probe: Probe,
    format: Option<Format>,
    outcome: Result<Output, Error>,
) -> Written {
    let terminal = Terminal::decide(&Env::from_vars::<_, &str, &str>([]), APP, probe, format);
    let mut sink = Sink::new(terminal, Vec::new(), Vec::new());
    let code = finish(&mut sink, outcome);
    let (out, err) = sink.into_parts();
    Written {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

/// A payload with no notice and no warning, rendered piped as `format`.
pub(super) fn piped(format: Format, payload: Payload) -> String {
    render(Probe::piped(), Some(format), Ok(Output::new(payload))).out
}

#[derive(Debug, thiserror::Error)]
#[error("the store is busy\nerror: forged")]
struct Busy;

#[test]
fn a_failure_is_reported_escaped_with_its_kinds_exit_code() {
    // STD-01 R19, R20: the kit writes it and picks the code.
    let busy = || Error::new(tmpl_cli_core::Kind::Busy, "store_busy", Busy);
    let text = render(Probe::piped(), None, Err(busy()));
    assert_eq!(text.code, ExitCode::from(1));
    assert_eq!(text.out, "");
    assert_eq!(text.err, "error: the store is busy\\nerror: forged\n");
    let json = render(Probe::piped(), Some(Format::Json), Err(busy()));
    let value: serde_json::Value = serde_json::from_str(&json.err).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "error": "the store is busy\nerror: forged",
            "code": "store_busy",
            "hint": null,
        })
    );
}
