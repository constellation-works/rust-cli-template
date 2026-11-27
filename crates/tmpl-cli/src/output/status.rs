//! The `status` payload and its human view: the table the kit's sink
//! renders in table mode and as one tab-separated line otherwise.

use cw_cli::{Cell, Column, Role, Table};
use serde::Serialize;

/// The store's status as every output mode sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct StatusPayload {
    /// The data directory.
    pub(crate) root: String,
    /// The database's store format; `null` when there is no database yet,
    /// never omitted (R11).
    pub(crate) format: Option<u32>,
}

/// The store's status as one row, `ROOT  FORMAT`. A store with no database
/// yet has no format: `-` in the human modes, `null` in JSON (STD-01 R11,
/// R14).
pub(super) fn to_table(status: &StatusPayload) -> Table {
    let format = status.format.map_or_else(
        || Cell::absent().role(Role::Muted),
        |format| Cell::new(format.to_string()),
    );
    Table::new([Column::new("ROOT"), Column::new("FORMAT").right()])
        .row([Cell::new(status.root.clone()), format])
}
