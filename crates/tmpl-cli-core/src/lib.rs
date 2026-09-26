//! The tmpl-cli note store as a library.
//!
//! This crate is the domain, split by responsibility so it can grow:
//!
//! - business logic with no I/O: the note types ([`Note`], [`NoteId`],
//!   [`Title`], [`Body`], [`Tag`], [`Priority`]) and their rules in `note`,
//!   and list selection ([`ListFilter`], [`NoteList`]) in `query`;
//! - persistence in `store`: the file-backed [`Store`], its format, the audit
//!   log ([`AuditEvent`]) and the filesystem mechanisms, and nothing else;
//! - the crate's typed [`Error`].
//!
//! It never parses arguments, never touches the terminal and never decides
//! where data lives: the caller resolves the data directory and hands it to
//! [`Store::open`] (STD-02 R3). Anything another surface (an MCP server, a
//! desktop app) would need to see the same notes belongs here, not in the CLI
//! crate.

// Library code never prints; diagnostics go through `tracing` (STD-02 R15).
#![deny(clippy::print_stderr, clippy::print_stdout)]
// Unit tests use unwrap/expect for fixture setup; production call sites remain linted.
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

mod error;
mod note;
mod query;
mod store;

pub use error::{Error, Result};
pub use note::{Body, NewNote, Note, NoteError, NoteId, Priority, Tag, Title};
pub use query::{ListFilter, NoteList};
pub use store::{AUDIT_FILE, AuditEvent, AuditStatus, DEFAULT_LOCK_WAIT, STORE_FILE, Store};

#[cfg(test)]
mod tests;
