//! One module per storage mechanism, each holding only this project's part
//! of it: the kit crate is the mechanism.
//!
//! A driver sets its mechanism up once and returns a handle; it holds no
//! query and knows no record's rules. A repository (`crate::repository`) is
//! one record's use of that handle.
//!
//! - `sqlite`: the database: its file and lock wait in `mod.rs`, and
//!   in `schema/` one module per table and the one ordered step list,
//!   opened as a `cw_sqlite::Db`.

pub(crate) mod sqlite;
