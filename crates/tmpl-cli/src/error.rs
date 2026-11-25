//! What the surfaces decide about a failure: the remedy to print, and their
//! own failures (output that could not be written, a command line that does
//! not parse, an MCP session that could not be served).
//!
//! Every failure a command can end with is the workspace's one error type,
//! `tmpl_cli_core::Error` (STD-02 R11). Units translate into it beside their
//! own typed errors (STD-02 R12), so nothing here converts one. How a
//! failure is shown and the exit code its kind calls for are the kit's:
//! `cw_cli::Sink::report` writes it and returns its `cw_cli::Exit`.

use tmpl_cli_core::{Error, Kind};

/// The surface's own failures.
#[derive(Debug, thiserror::Error)]
enum SurfaceError {
    /// The kit could not write the output.
    #[error(transparent)]
    Output(cw_cli::Error),
    /// The command line does not parse: clap's own sentence.
    #[error("{0}")]
    Usage(String),
    /// A payload could not be encoded as a tool's JSON.
    #[error("cannot encode the result as JSON")]
    Encode(#[source] serde_json::Error),
    /// The MCP server could not be built or served.
    #[error(transparent)]
    Mcp(cw_mcp::Error),
}

impl SurfaceError {
    /// A stable `snake_case` code for this failure (STD-01 R19).
    fn code(&self) -> &'static str {
        match self {
            Self::Output(cw_cli::Error::Output(_)) => "output_error",
            Self::Output(cw_cli::Error::Encode(_)) => "output_encode_error",
            Self::Output(cw_cli::Error::Summaries { .. }) => "help_summaries",
            Self::Usage(_) => "usage_error",
            Self::Encode(_) => "output_encode_error",
            Self::Mcp(cw_mcp::Error::DuplicateTool { .. } | cw_mcp::Error::InputSchema { .. }) => {
                "mcp_tool_list"
            }
            Self::Mcp(cw_mcp::Error::Panicked { .. }) => TOOL_PANICKED,
            Self::Mcp(cw_mcp::Error::Runtime(_)) => "mcp_runtime",
            Self::Mcp(cw_mcp::Error::Connection(_)) => "mcp_connection",
        }
    }

    /// How this failure is classified.
    fn kind(&self) -> Kind {
        match self {
            Self::Output(err) => err.kind(),
            Self::Usage(_) => Kind::Usage,
            Self::Encode(_) => Kind::Internal,
            Self::Mcp(err) => err.kind(),
        }
    }
}

/// The surface's one translator into the workspace's error type
/// (STD-02 R12).
impl From<SurfaceError> for Error {
    fn from(err: SurfaceError) -> Self {
        Self::new(err.kind(), err.code(), err)
    }
}

/// An output failure as the workspace's error type.
pub(crate) fn output_error(source: cw_cli::Error) -> Error {
    SurfaceError::Output(source).into()
}

/// A payload that could not become a tool's JSON, as the workspace's
/// error type.
pub(crate) fn encode_error(source: serde_json::Error) -> Error {
    SurfaceError::Encode(source).into()
}

/// The MCP server's own failure, as the workspace's error type.
pub(crate) fn mcp_error(source: cw_mcp::Error) -> Error {
    SurfaceError::Mcp(source).into()
}

/// The code an MCP tool that panicked is answered with.
pub(crate) const TOOL_PANICKED: &str = "tool_panicked";

/// A command line that is not usable as typed, as the workspace's error
/// type, which `Sink::report` exits 2 for: clap's sentence without its
/// `error: ` prefix, or a check of the surface's own that clap cannot
/// express (`GlobalArgs::check`). A command handler raises one the same
/// way.
pub(crate) fn usage_error(message: &str) -> Error {
    SurfaceError::Usage(message.to_owned()).into()
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
