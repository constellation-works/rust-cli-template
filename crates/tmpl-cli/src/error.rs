//! What the surface decides about a failure: the exit code, the remedy to
//! print, and its own one failure (output that could not be written).
//!
//! Every failure a command can end with is the workspace's one error type,
//! `tmpl_cli_core::Error` (STD-02 R11). Units translate into it beside their
//! own typed errors (STD-02 R12), so nothing here converts one. Usage errors
//! never reach it: clap reports them and exits 2 before a command runs.

use std::io;
use tmpl_cli_core::{Error, Kind};

/// Writing the result to stdout failed for a reason other than a closed pipe.
#[derive(Debug, thiserror::Error)]
#[error("cannot write output")]
struct OutputError(#[source] io::Error);

/// The surface's one translator: an output failure as the workspace's error
/// type (STD-02 R12).
pub(crate) fn output_error(source: io::Error) -> Error {
    Error::new(Kind::Io, "output_error", OutputError(source))
}

/// The process exit code. Every kind of command failure exits 1; only usage
/// errors exit 2 (STD-01 R20). clap reports today's usage errors before a
/// command runs, so no command fails with `Kind::Usage` yet; one that does
/// exits 2 like clap. Exhaustive, so a new kind decides here, and a further
/// code would need documenting in the command's help.
pub(crate) fn exit_code(err: &Error) -> u8 {
    match err.kind() {
        Kind::Usage => 2,
        Kind::NotFound
        | Kind::Refused
        | Kind::Busy
        | Kind::Corrupt
        | Kind::Incompatible
        | Kind::Config
        | Kind::Io
        | Kind::Internal => 1,
    }
}

/// Each fix this surface knows, keyed on the error's stable code (STD-02
/// R10). A remedy is spelled in this surface's commands, so it lives here,
/// and must be runnable as written (STD-02 R26): each keeps the reader on
/// the same data directory.
const HINTS: &[(&str, &str)] = &[
    // example:begin
    (
        "note_not_found",
        "run 'tmpl-cli note list' with the same --root to see existing ids",
    ),
    // example:end
];

/// The fix, where it is known and not already in the message (STD-01 R21).
pub(crate) fn hint(err: &Error) -> Option<&'static str> {
    let code = err.code();
    HINTS
        .iter()
        .find(|(known, _)| *known == code)
        .map(|(_, hint)| *hint)
}
