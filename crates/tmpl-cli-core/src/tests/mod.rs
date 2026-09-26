//! Unit tests for the business-logic modules, one file per source file
//! (STD-02 R19). Being siblings rather than children of the modules they test,
//! they reach only `pub(crate)` items. The store's tests live in
//! `store/tests/`.

mod note;
mod query;
