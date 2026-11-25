//! `note add`: assign the next id and store the note.

use super::rule::{self, NewNote};
use super::{Error, Outcome};
use crate::runtime::Ctx;

/// Load the notes, refuse what can be refused before any side effect, then
/// assign the next id and store the note in one write.
pub(super) fn add(ctx: &Ctx<'_>, new: NewNote) -> Result<Outcome, Error> {
    let now = ctx.now();
    let store = ctx.store();
    // Every check that can refuse runs before the first write (STD-02 R34):
    // an unsafe, corrupt, newer, bad-record or id-exhausted store is refused
    // here, before the directory or the database is created.
    // The snapshot serves only these checks, so it is closed before the
    // write begins rather than holding its read transaction open across it.
    {
        let snapshot = store.read()?;
        let notes = snapshot.notes();
        if let Some(bad) = notes.bad_records()?.first() {
            return Err(bad.refusal().into());
        }
        if rule::next_id(&notes.all()?).is_none() {
            return Err(Error::IdExhausted { path: store.path() });
        }
    }
    // The id is decided again inside the write, against what the store holds
    // under its write lock: anything read before it may be stale.
    // Inside the write the flow's own refusal is translated into the
    // workspace's error type, which is what the store's write returns; its
    // code and its typed form (by `downcast_ref`) are kept.
    let note = store.write(|tx| {
        let id = rule::next_id(&tx.notes().all()?)
            .ok_or_else(|| cw_base::Error::from(Error::IdExhausted { path: store.path() }))?;
        let note = new.into_note(id, now);
        tx.notes().put(note.clone())?;
        Ok(note)
    })?;
    let path = store.path();
    tracing::debug!(id = %note.id, path = %path.display(), "added note");
    tracing::info!(id = %note.id, "note added");
    Ok(Outcome::Added { note, path })
}
