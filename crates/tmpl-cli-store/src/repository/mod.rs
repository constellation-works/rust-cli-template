//! One repository per record: `notes` and `audit`.
//!
//! A repository is named for the record it holds, never for a command: it
//! gets, lists and puts records and decides nothing about them. A read
//! returns a [`Snapshot`]; a write runs against a [`Tx`] inside
//! [`crate::Store::write`], so a caller composes repositories in one read or
//! one write.

mod audit;
mod notes;

pub use audit::Audit;
pub use notes::{Notes, NotesMut};

use crate::db::format::StoreDocument;

/// The store as one read saw it.
#[derive(Debug)]
pub struct Snapshot {
    doc: StoreDocument,
}

impl Snapshot {
    pub(crate) fn new(doc: StoreDocument) -> Self {
        Self { doc }
    }

    /// The notes repository over this snapshot.
    #[must_use]
    pub fn notes(&self) -> Notes<'_> {
        Notes::new(&self.doc)
    }
}

/// The store inside one write, under the store lock.
#[derive(Debug)]
pub struct Tx<'a> {
    doc: &'a mut StoreDocument,
}

impl<'a> Tx<'a> {
    pub(crate) fn new(doc: &'a mut StoreDocument) -> Self {
        Self { doc }
    }

    /// The notes repository inside this write.
    pub fn notes(&mut self) -> NotesMut<'_> {
        NotesMut::new(self.doc)
    }
}
