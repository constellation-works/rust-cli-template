//! The tmpl-cli data directory as a library.
//!
//! - `db`: the files under the data directory (the persisted notes document
//!   and its format upgrades, the audit line); bytes only, no I/O;
//! - [`repository`]: one repository per record (notes, audit), read through a
//!   [`Snapshot`] and written through a [`Tx`] inside [`Store::write`];
//! - [`contracts`]: the record shapes ([`contracts::Note`] and its field
//!   types, [`contracts::AuditEvent`]);
//! - the crate's typed [`Error`], and its one translator into the
//!   workspace's error type.
//!
//! It reaches the data directory only through `tmpl_cli_common::fs`: no code
//! in this crate names `std::fs`.
//!
//! The store holds no rule of any app: no next-id rule, no list selection,
//! no validation beyond what a contract type enforces on itself. It never
//! parses arguments, never touches the terminal and never decides where data
//! lives: the caller resolves the data directory and hands it to
//! [`Store::open`] (STD-02 R3).

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

pub mod contracts;
mod db;
mod error;
pub mod repository;
mod store;

pub use db::STORE_FILE;
pub use error::{BadRecord, BadRecordReason, Error, RecordLocator, Result};
pub use repository::{Snapshot, Tx};
pub use store::Store;

#[cfg(test)]
mod tests;
