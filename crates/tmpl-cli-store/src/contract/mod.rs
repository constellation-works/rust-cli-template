//! Record shapes: what the store persists and what reads return.
//!
//! Type definitions only, each with the constructor or parser that enforces
//! its own invariant, serde and `Display`. No function here decides anything
//! about two records, reads the clock or touches a file
//! (`scripts/check-dependency-direction.sh`).

// example:begin
mod note;

pub use note::{Body, Note, NoteError, NoteId, Priority, Tag, Title};
// example:end
