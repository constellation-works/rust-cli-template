//! The notes repository: note records by id.

use crate::contracts::{Note, NoteId};
use crate::db::format::StoreDocument;
use crate::error::{BadRecord, RecordLocator};

/// Note records as one read saw them.
#[derive(Clone, Copy, Debug)]
pub struct Notes<'a> {
    doc: &'a StoreDocument,
}

impl<'a> Notes<'a> {
    pub(crate) fn new(doc: &'a StoreDocument) -> Self {
        Self { doc }
    }

    /// Every valid note, in stored (oldest-first) order.
    #[must_use]
    pub fn all(&self) -> &'a [Note] {
        &self.doc.notes
    }

    /// The valid note with this id.
    #[must_use]
    pub fn get(&self, id: NoteId) -> Option<&'a Note> {
        self.doc.notes.iter().find(|n| n.id == id)
    }

    /// Every note record that failed validation, isolated from the valid
    /// ones (STD-02 R32); empty when every record was valid.
    #[must_use]
    pub fn bad_records(&self) -> &'a [BadRecord] {
        &self.doc.bad_records
    }

    /// The bad record with this id, when the record's id was readable.
    #[must_use]
    pub fn bad_record(&self, id: NoteId) -> Option<&'a BadRecord> {
        self.doc
            .bad_records
            .iter()
            .find(|r| r.locator == RecordLocator::Id(id))
    }
}

/// Note records inside one write.
#[derive(Debug)]
pub struct NotesMut<'a> {
    doc: &'a mut StoreDocument,
}

impl<'a> NotesMut<'a> {
    pub(crate) fn new(doc: &'a mut StoreDocument) -> Self {
        Self { doc }
    }

    /// Every valid note, in stored (oldest-first) order, as this write sees it.
    #[must_use]
    pub fn all(&self) -> &[Note] {
        &self.doc.notes
    }

    /// Store `note`: replace the note with its id, or append it when the id
    /// is new.
    pub fn put(&mut self, note: Note) {
        match self.doc.notes.iter_mut().find(|n| n.id == note.id) {
            Some(existing) => *existing = note,
            None => self.doc.notes.push(note),
        }
    }
}
