//! `note list`: the notes matching a filter.

use super::rule::{self, ListFilter};
use super::{Error, Outcome};
use crate::composition::Ctx;

/// Notes matching `filter`, oldest first, how many matched, and any bad
/// records isolated from the store. A missing store is an empty one.
pub(super) fn list(ctx: &Ctx<'_>, filter: ListFilter) -> Result<Outcome, Error> {
    let snapshot = ctx.state().read()?;
    let notes = snapshot.notes();
    Ok(Outcome::Listed {
        list: rule::select(notes.all()?, &filter),
        filter,
        bad_records: notes.bad_records()?,
    })
}
