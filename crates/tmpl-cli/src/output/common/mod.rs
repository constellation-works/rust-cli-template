//! The payloads two or more commands share, written through the kit's sink.
//!
//! Each top-level command keeps its own file directly under `output/` for
//! its payloads and its human view; what commands share lives here, in files
//! named for what they hold (STD-02 R48). `common::payload` holds the
//! envelope every mode renders.

pub(crate) mod payload;
