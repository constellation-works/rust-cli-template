//! The tmpl-cli foundation: what every other crate may name, and what names
//! none of them.
//!
//! - [`error`]: the one error type every unit translates its own failure
//!   into ([`Error`]) and how a failure is classified ([`Kind`]);
//! - [`fs`]: file access for a private data directory (durable writes,
//!   appends, locks, owner-only checks); the only code in the workspace that
//!   names `std::fs`.
//!
//! It is grouped by subject in a folder each and depends on no
//! other crate of the workspace. `error` does no I/O, reads no clock and
//! never imports `fs`; nothing here reads the environment
//! (`scripts/check-dependency-direction.sh`).

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

pub mod error;
pub mod fs;

pub use error::{Error, Kind};
