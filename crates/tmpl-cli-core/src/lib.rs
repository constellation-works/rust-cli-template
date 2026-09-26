//! The tmpl-cli note store as a library.
//!
//! This crate is the domain: note types ([`Note`], [`NoteId`], [`Title`],
//! [`Body`], [`Tag`], [`Priority`]), the file-backed [`Store`], and the crate's typed
//! [`Error`]. It never parses arguments, never touches the terminal and never
//! decides where data lives: the caller resolves the data directory and hands
//! it to [`Store::open`] (STD-02 R3). Anything another surface (an MCP server,
//! a desktop app) would need to see the same notes belongs here, not in the
//! CLI crate.

// Library code never prints; diagnostics go through `tracing` (STD-02 R15).
#![deny(clippy::print_stderr, clippy::print_stdout)]
// Unit tests use unwrap/expect for fixture setup; production call sites remain linted.
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

mod error;
mod fsio;
mod note;
mod store;

pub use error::{Error, Result};
pub use note::{Body, NewNote, Note, NoteError, NoteId, Priority, Tag, Title};
pub use store::{DEFAULT_LOCK_WAIT, ListFilter, NoteList, STORE_FILE, Store};

#[cfg(test)]
mod tests;
