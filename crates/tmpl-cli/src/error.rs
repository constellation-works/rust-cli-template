//! The surface error type: every failure a command can end with.
//!
//! Domain errors arrive through the one translator, `#[from]` on
//! [`CliError::Core`], applied by `?` (STD-02 R12); nothing matches core
//! variants ad hoc. Usage errors never reach this type: clap reports them
//! and exits 2 before a command runs.

use std::io;

/// Why a command failed. Every variant exits 1 (STD-01 R20).
#[derive(Debug, thiserror::Error)]
pub(crate) enum CliError {
    /// The store refused or failed.
    #[error(transparent)]
    Core(#[from] tmpl_cli_core::Error),
    /// No data directory could be resolved.
    #[error("no data directory: pass --root <dir> or set TMPL_CLI_ROOT (HOME is not set either)")]
    NoDataDir,
    /// Writing the result to stdout failed for a reason other than a closed pipe.
    #[error("cannot write output")]
    Output(#[source] io::Error),
}

impl CliError {
    /// Stable `snake_case` code for the JSON error object (STD-01 R19).
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::Core(e) => e.code(),
            Self::NoDataDir => "no_data_dir",
            Self::Output(_) => "output_error",
        }
    }

    /// The process exit code. Only usage errors (clap's) exit 2.
    pub(crate) fn exit_code(&self) -> u8 {
        1
    }

    /// The fix, where it is known and not already in the message (STD-01
    /// R21). A remedy must be runnable as written (STD-02 R26), so this one
    /// reminds the reader to keep the same data directory.
    pub(crate) fn hint(&self) -> Option<&'static str> {
        match self {
            Self::Core(tmpl_cli_core::Error::NotFound { .. }) => {
                Some("run 'tmpl-cli note list' with the same --root to see existing ids")
            }
            // The core enum is #[non_exhaustive], so it needs this arm.
            Self::Core(_) | Self::NoDataDir | Self::Output(_) => None,
        }
    }
}
