//! Composition (STD-02 R2): the one place that resolves configuration, picks
//! the data directory and constructs the store. It hands the domain resolved
//! values (STD-02 R3) and knows nothing about argument parsing or rendering;
//! `scripts/check-dependency-direction.sh` rejects `crate::cli`,
//! `crate::command` and `crate::output` imports here.

use crate::error::CliError;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use tmpl_cli_core::Store;

/// Environment variable naming the data directory.
pub(crate) const ROOT_ENV: &str = "TMPL_CLI_ROOT";

/// Directory under `$HOME` used when nothing else names one.
const DEFAULT_DIR: &str = ".tmpl-cli";

/// The configuration-relevant environment, captured once.
#[derive(Clone, Debug, Default)]
pub(crate) struct AppEnv {
    /// `TMPL_CLI_ROOT`.
    pub(crate) root: Option<OsString>,
    /// `HOME`.
    pub(crate) home: Option<OsString>,
}

impl AppEnv {
    /// Read the variables from the real process environment.
    pub(crate) fn from_process() -> Self {
        Self {
            root: std::env::var_os(ROOT_ENV),
            home: std::env::var_os("HOME"),
        }
    }
}

/// Everything a command needs, resolved.
#[derive(Debug)]
pub(crate) struct Context {
    store: Store,
}

impl Context {
    /// Resolve the context. Data directory precedence: `--root` >
    /// `TMPL_CLI_ROOT` > `$HOME/.tmpl-cli`.
    pub(crate) fn resolve(root_flag: Option<&Path>, env: &AppEnv) -> Result<Self, CliError> {
        let root = resolve_root(root_flag, env)?;
        tracing::debug!(root = %root.display(), "resolved data directory");
        Ok(Self {
            store: Store::open(root),
        })
    }

    /// The note store.
    pub(crate) fn store(&self) -> &Store {
        &self.store
    }

    /// The current time, whole seconds, for records created by this invocation.
    pub(crate) fn now(&self) -> OffsetDateTime {
        let now = OffsetDateTime::now_utc();
        now.replace_nanosecond(0).unwrap_or(now)
    }
}

/// The data-directory precedence, separated so tests can check it directly.
pub(crate) fn resolve_root(root_flag: Option<&Path>, env: &AppEnv) -> Result<PathBuf, CliError> {
    let non_empty = |v: &Option<OsString>| v.clone().filter(|s| !s.is_empty());
    if let Some(flag) = root_flag {
        return Ok(flag.to_path_buf());
    }
    if let Some(root) = non_empty(&env.root) {
        return Ok(PathBuf::from(root));
    }
    non_empty(&env.home)
        .map(|home| PathBuf::from(home).join(DEFAULT_DIR))
        .ok_or(CliError::NoDataDir)
}
