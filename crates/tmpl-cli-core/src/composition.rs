//! Composition (STD-02 R2): the one place in the core that reads the host.
//!
//! [`Env`] is the environment captured once by the binary's `main`, plus the
//! `--root` flag the surface was given. From it composition picks the data
//! directory, opens the [`Store`], names the actor and supplies the clock.
//! Nothing else in the core reads an environment variable or discovers a
//! path (`scripts/check-dependency-direction.sh`).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use tmpl_cli_common::Kind;
use tmpl_cli_store::Store;

/// Why composition could not resolve what an app runs against.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Error {
    /// No data directory could be resolved.
    #[error("no data directory: pass --root <dir> or set TMPL_CLI_ROOT (HOME is not set either)")]
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
impl From<Error> for tmpl_cli_common::Error {
    fn from(err: Error) -> Self {
        Self::new(err.kind(), err.code(), err)
    }
}

/// Environment variable naming the data directory.
const ROOT_ENV: &str = "TMPL_CLI_ROOT";

/// Directory under `$HOME` used when nothing else names one.
const DEFAULT_DIR: &str = ".tmpl-cli";

/// The configuration-relevant environment, captured once, and the data
/// directory the command line named.
#[derive(Clone, Debug, Default)]
pub struct Env {
    /// `--root`, when the command line gave it.
    pub(crate) root_flag: Option<PathBuf>,
    /// `TMPL_CLI_ROOT`.
    pub(crate) root: Option<OsString>,
    /// `HOME`.
    pub(crate) home: Option<OsString>,
    /// `USER`.
    pub(crate) user: Option<OsString>,
    /// `USERNAME` (fallback when `USER` is unset or empty).
    pub(crate) username: Option<OsString>,
}

impl Env {
    /// Read the variables from the real process environment.
    #[must_use]
    pub fn from_process() -> Self {
        Self {
            root_flag: None,
            root: std::env::var_os(ROOT_ENV),
            home: std::env::var_os("HOME"),
            user: std::env::var_os("USER"),
            username: std::env::var_os("USERNAME"),
        }
    }

    /// The same environment with the data directory the command line named,
    /// which beats every variable.
    #[must_use]
    pub fn with_root_flag(mut self, root: Option<PathBuf>) -> Self {
        self.root_flag = root;
        self
    }
}

/// What an app runs against, resolved from an [`Env`].
#[derive(Debug)]
pub(crate) struct Composition {
    /// The note store at the resolved data directory.
    pub(crate) store: Store,
    /// The OS user name, for the audit log.
    pub(crate) actor: Option<String>,
}

/// Resolve everything an app needs. Data directory precedence: `--root` >
/// `TMPL_CLI_ROOT` > `$HOME/.tmpl-cli`.
pub(crate) fn compose(env: &Env) -> Result<Composition, Error> {
    let root = resolve_root(env.root_flag.as_deref(), env)?;
    let actor = resolve_actor(env);
    tracing::debug!(root = %root.display(), "resolved data directory");
    Ok(Composition {
        store: Store::open(root),
        actor,
    })
}

/// The current time, whole seconds, for records created by this invocation.
pub(crate) fn now() -> OffsetDateTime {
    let now = OffsetDateTime::now_utc();
    now.replace_nanosecond(0).unwrap_or(now)
}

/// The data-directory precedence, separated so tests can check it directly.
pub(crate) fn resolve_root(root_flag: Option<&Path>, env: &Env) -> Result<PathBuf, Error> {
    let non_empty = |v: &Option<OsString>| v.clone().filter(|s| !s.is_empty());
    if let Some(flag) = root_flag {
        return Ok(flag.to_path_buf());
    }
    if let Some(root) = non_empty(&env.root) {
        return Ok(PathBuf::from(root));
    }
    non_empty(&env.home)
        .map(|home| PathBuf::from(home).join(DEFAULT_DIR))
        .ok_or(Error::NoDataDir)
}

/// The actor precedence (`USER`, else `USERNAME`, else `None`), separated so
/// tests can check it directly without touching the process environment.
pub(crate) fn resolve_actor(env: &Env) -> Option<String> {
    let non_empty = |v: &Option<OsString>| {
        v.as_deref()
            .and_then(std::ffi::OsStr::to_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    non_empty(&env.user).or_else(|| non_empty(&env.username))
}
