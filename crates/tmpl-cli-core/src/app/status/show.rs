//! `status`: the data directory and the store format, from one read.

use super::{Error, Outcome};
use crate::runtime::Ctx;

/// The data directory and the format of the database in it. A data
/// directory or database that does not exist yet is reported, not created.
pub(super) fn show(ctx: &Ctx<'_>) -> Result<Outcome, Error> {
    let store = ctx.store();
    let snapshot = store.read()?;
    Ok(Outcome::Shown {
        root: store.root().to_path_buf(),
        format: snapshot.format(),
    })
}
