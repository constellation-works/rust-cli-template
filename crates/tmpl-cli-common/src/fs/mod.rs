//! File access for a private data directory: durable writes, appends,
//! advisory locks and owner-only checks (STD-03 R4–R7, STD-05 R7–R9).
//!
//! Mechanism only. It knows no record and no file name: a caller hands it a
//! path and bytes. It is the only code in the workspace that names
//! `std::fs`; the store reaches the data directory through it, and so does
//! any later unit that owns a file of its own
//! (`scripts/check-dependency-direction.sh`).
//!
//! - `fsio`: the operations ([`write_atomic`], [`read`], [`append_private`],
//!   [`list_dir`], [`remove_private`], [`create_private_dir`],
//!   [`inspect_dir`], [`inspect_file`], [`LockGuard`]);
//! - `error`: the typed [`Error`] they fail with. The unit that calls them
//!   wraps it in its own error and gives it a stable code.

mod error;
mod fsio;

pub use error::{Error, Result};
pub use fsio::{
    LockGuard, append_private, create_private_dir, inspect_dir, inspect_file, list_dir, read,
    remove_private, write_atomic,
};

#[cfg(test)]
mod tests;
