//! The note app's rules: the next id, building a stored note from a new one,
//! and which notes a listing returns (filter, count, then limit).
//!
//! Pure functions with no I/O (STD-02 R5): a flow loads the notes from the
//! store and hands them here, so the rules are tested without a filesystem
//! and any future backend applies the same ones.
//! `scripts/check-dependency-direction.sh` rejects file access and any use of
//! the store or the runtime here.

use std::num::NonZeroUsize;
use time::OffsetDateTime;
use tmpl_cli_store::contracts::{Body, Note, NoteId, Priority, Tag, Title};

/// The fields a caller supplies to add a note; the app assigns the rest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewNote {
    /// One line of text.
    pub title: Title,
    /// Optional longer text.
    pub body: Option<Body>,
    /// Labels, in any order; a repeated tag is stored once.
    pub tags: Vec<Tag>,
    /// How much it matters.
    pub priority: Priority,
}

impl NewNote {
    /// Build the stored note, with its tags sorted and deduplicated.
    pub(crate) fn into_note(self, id: NoteId, created_at: OffsetDateTime) -> Note {
        let mut tags = self.tags;
        tags.sort();
        tags.dedup();
        Note {
            id,
            title: self.title,
            body: self.body.map(String::from),
            tags,
            priority: self.priority,
            created_at,
        }
    }
}

/// The id to assign to a note added after `notes`: one past the highest, or
/// [`NoteId::FIRST`] when there are none, or `None` when the highest id is
/// already `u64::MAX`, so an existing id is never handed out again.
pub(crate) fn next_id(notes: &[Note]) -> Option<NoteId> {
    match notes.iter().map(|n| n.id).max() {
        None => Some(NoteId::FIRST),
        Some(max) => max
            .get()
            .checked_add(1)
            .and_then(|next| NoteId::try_from(next).ok()),
    }
}

/// Which notes a `note list` returns.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListFilter {
    /// Only notes carrying this tag.
    pub tag: Option<Tag>,
    /// Only notes at this priority.
    pub priority: Option<Priority>,
    /// At most this many of the matching notes (the oldest first).
    pub limit: Option<NonZeroUsize>,
}

impl ListFilter {
    fn matches(&self, note: &Note) -> bool {
        self.tag.as_ref().is_none_or(|t| note.tags.contains(t))
            && self.priority.is_none_or(|p| note.priority == p)
    }
}

/// What a listing found: the notes returned and how many matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteList {
    /// The matching notes, oldest first, cut to the filter's limit.
    pub notes: Vec<Note>,
    /// How many notes matched before the limit was applied.
    pub total: usize,
}

impl NoteList {
    /// Whether the limit left matching notes out (STD-01 R34).
    #[must_use]
    pub fn truncated(&self) -> bool {
        self.notes.len() < self.total
    }
}

/// Apply `filter` to `notes`, kept in their stored (oldest-first) order. The
/// filters apply before the limit, and `total` counts every match, so a cut
/// list can say how much it left out (STD-01 R33, R34).
pub(crate) fn select(notes: Vec<Note>, filter: &ListFilter) -> NoteList {
    let mut notes: Vec<Note> = notes.into_iter().filter(|n| filter.matches(n)).collect();
    let total = notes.len();
    if let Some(limit) = filter.limit {
        notes.truncate(limit.get());
    }
    NoteList { notes, total }
}
