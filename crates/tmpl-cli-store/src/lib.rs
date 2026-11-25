//! The tmpl-cli data directory as a library.
//!
//! - `driver`: one module per storage mechanism, each holding only this
//!   project's part of it. `driver::sqlite` names the notes database, its
//!   lock wait and its schema steps, and hands out the kit's
//!   `cw_sqlite::Db`; it holds no query;
//! - [`repository`]: one repository per record (notes, audit), read through a
//!   [`Snapshot`] and written through a [`Tx`] inside [`Store::write`]; the
//!   notes repository holds the notes' SQL and row mapping;
//! - [`contract`]: the record shapes ([`contract::Note`] and its field
//!   types, [`contract::AuditEvent`]);
//! - `db`: the audit log's file and its persisted line;
//! - the crate's typed [`Error`], and its one translator into the
//!   workspace's error type.
//!
//! It reaches the notes database only through the kit's `cw_sqlite` and the
//! data directory and the audit log only through the kit's `cw_fs`: no code
//! in this crate names `std::fs`. No type of either crate, no path but
//! [`Store::path`] and [`Store::audit_path`], and no borrow from a
//! connection is in its public surface.
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

pub mod contract;

mod driver;

mod error;

pub mod repository;

mod store;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use driver::sqlite::STORE_FILE;
pub use error::{BadRecord, BadRecordReason, Error, RecordLocator, Result};
pub use repository::{Snapshot, Tx};
pub use store::Store;

#[cfg(test)]
mod tests;
