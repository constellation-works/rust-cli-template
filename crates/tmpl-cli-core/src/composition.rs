//! Composition (STD-02 R2): what every command runs against, built once
//! from the environment the binary's `main` captured.
//!
//! [`start`] resolves the data directory from the captured [`Env`] and the
//! `--root` the surface was given, opens the [`Store`] there, and builds the
//! kit's [`cw_app::Runtime`] over it, which audits to the same directory and
//! reads the system clock. The process environment is read once, by
//! `Env::capture()` in `main`; everything here is a pure function of that
//! value, so a test builds one with `Env::from_vars` (`clippy.toml` refuses
//! any other read of the process).

use cw_app::Env;
use cw_app::env::{HOME, ROOT};
use cw_base::Kind;
use std::path::{Path, PathBuf};
use tmpl_cli_store::Store;

/// The program's name. The kit derives its variables from it
/// (`TMPL_CLI_ROOT`, `TMPL_CLI_FORMAT`, `TMPL_CLI_LOG`) and its default data
/// directory (`$HOME/.tmpl-cli`).
pub const APP: &str = "tmpl-cli";

/// What every command runs against: the store, the data directory the
/// audit log lives in, and the clock. A test pins the clock with
/// `Runtime::with_clock(Clock::fixed(..))`.
pub type Runtime = cw_app::Runtime<Store>;

/// What a flow runs against: the store, the clock and the client.
pub(crate) type Ctx<'a> = cw_app::Ctx<'a, Store>;

/// Why composition could not resolve what an app runs against.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Error {
    /// No data directory could be resolved.
    #[error(
        "no data directory: pass --root <dir> or set {} ({} is not set either)",
        ROOT.name_for(APP),
        HOME.name_for(APP)
    )]
    NoDataDir,
}

impl Error {
    /// A stable `snake_case` code for this failure, for machine output
    /// (STD-01 R19). Adding a code is fine; changing one is a breaking change.
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::NoDataDir => "no_data_dir",
        }
    }

    /// How this failure is classified.
    pub(crate) fn kind(&self) -> Kind {
        match self {
            Self::NoDataDir => Kind::Config,
        }
    }
}

/// Composition's one translator into the workspace's error type
/// (STD-02 R12).
impl From<Error> for cw_base::Error {
    fn from(err: Error) -> Self {
        Self::new(err.kind(), err.code(), err)
    }
}

/// Build the runtime every command of this invocation runs against.
/// Data directory precedence: `root` (the `--root` flag) > `TMPL_CLI_ROOT` >
/// `$HOME/.tmpl-cli`. Creates nothing: the store creates the directory on
/// its first write, the audit log on its first line.
///
/// # Errors
///
/// `no_data_dir` when nothing names a data directory.
pub fn start(env: &Env, root: Option<&Path>) -> Result<Runtime, cw_base::Error> {
    let root = data_dir(env, root)?;
    tracing::debug!(root = %root.display(), "resolved data directory");
    Ok(Runtime::new(&root, Store::open(&root)))
}

/// The data directory, resolved by the kit from the captured environment.
pub(crate) fn data_dir(env: &Env, root: Option<&Path>) -> Result<PathBuf, Error> {
    // The kit's resolution fails only when nothing names a directory; the
    // template keeps its own code and message for that, naming its flag.
    env.app(APP)
        .data_dir(root)
        .map_err(|_no_data_dir| Error::NoDataDir)
}
