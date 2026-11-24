//! The failures every app shares: the runtime could not start, or the store
//! refused or failed.
//!
//! Callers branch on the variant (or on [`Error::code`]), never on the
//! message (STD-02 R10). Store failures cross in through the one `#[from]`
//! and keep their own message and source.

/// Why an app could not run, whatever the app.
///
/// `#[non_exhaustive]`: this is the library's public boundary (STD-02 R11).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// No data directory could be resolved.
    #[error("no data directory: pass --root <dir> or set TMPL_CLI_ROOT (HOME is not set either)")]
    NoDataDir,
    /// The store refused or failed.
    #[error(transparent)]
    Store(#[from] tmpl_cli_store::Error),
}

impl Error {
    /// A stable `snake_case` code for this failure, for machine output
    /// (STD-01 R19). Adding a code is fine; changing one is a breaking change.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoDataDir => "no_data_dir",
            Self::Store(e) => e.code(),
        }
    }
}
