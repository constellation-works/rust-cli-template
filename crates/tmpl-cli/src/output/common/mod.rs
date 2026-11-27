//! Output machinery two or more commands share, written through the kit's
//! sink.
//!
//! Each top-level command keeps its own file directly under `output/` for
//! its payloads and its human view; what commands share lives here, in files
//! named for what they hold (STD-02 R48): `common::terminal` makes the
//! terminal decision and opens the sink, `common::payload` holds the
//! envelope every mode renders, `common::render` writes outcomes and
//! warnings through the sink, and `common::error` renders errors and usage
//! errors.

pub(crate) mod error;

pub(crate) mod payload;

pub(crate) mod render;

pub(crate) mod terminal;
