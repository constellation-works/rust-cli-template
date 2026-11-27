//! Captures the process environment once; imports no sibling modules.
//!
//! The one read of the process environment (STD-02 R7: no read of the
//! environment lives in the CLI crate).
//!
//! [`capture_env`] takes the kit's snapshot once; `main` calls it and hands
//! the [`Env`] down, and everything after is a pure function of that value.
//! `clippy.toml` refuses every other read of the process, and this module is
//! its one exemption for the environment.
#![expect(
    clippy::disallowed_methods,
    reason = "the one read of the process environment: cw_app::Env::capture()"
)]

use cw_app::Env;

/// The process environment, captured once. Call it only from `main`; a test
/// builds its `Env` with `Env::from_vars`.
#[must_use]
pub fn capture_env() -> Env {
    Env::capture()
}
