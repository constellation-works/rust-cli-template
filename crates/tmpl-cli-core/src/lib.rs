//! The tmpl-cli apps as a library.
//!
//! - [`app`]: one module per subject a user names on the command line
//!   (`status`, and any other), each implementing the kit's
//!   [`cw_app::App`] with its `Command`, `Outcome`, `Error`, flows and pure
//!   rules, and one `exec` that forwards to the kit's chokepoint,
//!   [`cw_app::exec`];
//! - `composition`: [`start`] builds the [`Runtime`] every command runs
//!   against from the environment `main` captured ([`Env`]): the data
//!   directory, the store, the clock and the audit log there.
//!
//! The command path is the kit's `cw-app`: its chokepoint starts the audit
//! record of a mutating command, runs it and returns a [`Finished`]; the
//! surface renders the result, then settles the record, and prints an
//! [`AuditWarning`] when the line could not be written. The audit log
//! (`<root>/audit.jsonl`) is written only by the kit.
//!
//! Persistence is the `tmpl-cli-store` crate; the record shapes it defines
//! are re-exported as [`contract`]. The one error type is the kit's,
//! `cw-base`, re-exported as [`Error`] and [`Kind`]. A surface so
//! names only this crate. This crate never parses arguments and never touches
//! the terminal. Anything another surface (an MCP server, a desktop app)
//! would need to see the same records belongs here, not in the CLI crate.

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

pub mod app;

mod composition;

pub use composition::{APP, Runtime, start};
pub use cw_app::{AuditWarning, Env, EnvVar, Finished, Origin, Surface};
pub use cw_base::{Error, Kind};
pub use tmpl_cli_store::contract;

#[cfg(test)]
mod tests;
