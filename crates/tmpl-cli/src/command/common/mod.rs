//! Command machinery two or more commands share.
//!
//! Each noun keeps its own file directly under `command/` for its arguments
//! and its handlers; what commands share lives here, in files named for what
//! they hold (STD-02 R49): `common::input` reads a tool's JSON text through
//! its type's own `FromStr`.

// example:begin
pub(crate) mod input;
// example:end
