use crate::output::{self, Output, Payload};
use cw_cli::{Format, Probe, Sink, Terminal};
use tmpl_cli_core::{APP, Env};

/// The stdout `payload` writes through the kit's sink, as a command's does,
/// for a reader `probe` describes asking for `format`.
pub(super) fn render(probe: Probe, format: Option<Format>, payload: Payload) -> String {
    let terminal = Terminal::decide(&Env::from_vars::<_, &str, &str>([]), APP, probe, format);
    let mut sink = Sink::new(terminal, Vec::new(), Vec::new());
    let _ = output::finish(&mut sink, Ok(Output::new(payload)));
    let (out, _err) = sink.into_parts();
    String::from_utf8(out).unwrap()
}

/// `payload` rendered piped as `format`.
pub(super) fn piped(format: Format, payload: Payload) -> String {
    render(Probe::piped(), Some(format), payload)
}
