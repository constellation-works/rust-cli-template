//! The tmpl-cli apps as a library.
//!
//! - [`apps`]: one module per subject a user names on the command line
//!   (today `note`), each with its `Command`, `Outcome`, `Error`, one `exec`,
//!   its flows and its pure rules;
//! - `runtime`: the context an app runs against, the one chokepoint every
//!   app's `exec` forwards to and the audit guard;
//! - `composition`: the one place that reads the host — the captured
//!   environment ([`Env`]), the data directory, the store and the clock.
//!
//! Persistence is the `tmpl-cli-store` crate; the record shapes it defines
//! are re-exported as [`contract`]. The one error type is the kit's,
//! `cw-base`, re-exported as [`Error`] and [`Kind`]. A surface so
//! names only this crate. This crate never parses arguments and never touches
//! the terminal. Anything another surface (an MCP server, a desktop app)
//! would need to see the same notes belongs here, not in the CLI crate.

// Library code never prints; diagnostics go through `tracing` (STD-02 R15).
#![deny(clippy::print_stderr, clippy::print_stdout)]
// Unit tests use unwrap/expect for fixture setup; production call sites remain linted.
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "unit tests assert fixture setup"
    )
)]

pub mod apps;
mod composition;
mod runtime;
#[cfg(feature = "test-support")]
pub mod test_support;

pub use composition::Env;
pub use cw_base::{Error, Kind};
pub use tmpl_cli_store::contract;

#[cfg(test)]
mod tests;
