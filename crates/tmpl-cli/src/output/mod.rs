//! Renders payloads and command views through the kit; imports error.
//!
//! The output layer: what this program shows, written through the kit.
//!
//! The mechanism is cw-cli's: one terminal decision ([`Terminal`]), the sink
//! that is the only path to stdout and stderr ([`Sink`]), error rendering
//! with the exit code from the error's kind ([`Sink::report`]), and the
//! table layout. stdout carries the payload and nothing else; notices,
//! warnings and errors go to stderr in every mode (STD-01 R12), and a closed
//! reader ends the writes, not the command (STD-01 R13). What stays here is
//! the program's own: its payloads, each command's human view (a table, or
//! the detail text of one record), and the colour role of each value.

mod common;

// example:begin
mod note;

// example:end
mod status;

pub(crate) use common::error::{fail, finish_parse_error};
pub(crate) use common::payload::{Output, Payload};
pub(crate) use common::render::{audit_warning, finish};
pub(crate) use common::terminal::{decide, sink};
// example:begin
pub(crate) use note::{NotePayload, SkippedRecord};
// example:end
pub(crate) use status::StatusPayload;

#[cfg(test)]
mod tests;
