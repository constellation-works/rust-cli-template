//! `note show`: one note by id.

use super::{Error, Outcome};
use crate::runtime::Ctx;
use tmpl_cli_store::contracts::NoteId;

/// The note with this id, plus any bad records isolated from other entries
/// in the store file. If `id` belongs to a bad record, fails with that
/// record's typed error.
pub(super) fn show(ctx: &Ctx<'_>, id: NoteId) -> Result<Outcome, Error> {
    let store = ctx.store();
    let snapshot = store.read()?;
    let notes = snapshot.notes();
    if let Some(bad) = notes.bad_record(id) {
        return Err(bad.refusal().into());
    }
    let note = notes.get(id).cloned().ok_or_else(|| Error::NotFound {
        id,
        path: store.path(),
    })?;
    Ok(Outcome::Shown {
        note,
        bad_records: notes.bad_records().to_vec(),
    })
}
