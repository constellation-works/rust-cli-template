//! `note list`: the notes matching a filter.

use super::rules::{self, ListFilter};
use super::{Error, Outcome};
use crate::runtime::Ctx;

/// Notes matching `filter`, oldest first, how many matched, and any bad
/// records isolated from the store. A missing store is an empty one.
pub(super) fn list(ctx: &Ctx<'_>, filter: ListFilter) -> Result<Outcome, Error> {
    let snapshot = ctx.store().read()?;
    let notes = snapshot.notes();
    Ok(Outcome::Listed {
        list: rules::select(notes.all()?, &filter),
        filter,
        bad_records: notes.bad_records()?,
    })
}
