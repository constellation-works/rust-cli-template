//! Which notes a listing returns: filter, count, then limit.
//!
//! Business logic with no I/O (STD-02 R5): the store loads the notes and hands
//! them here, so the selection rules are tested without a filesystem and any
//! future backend applies the same ones. `scripts/check-dependency-direction.sh`
//! rejects `std::fs`, `std::net`, `std::process` and `crate::store` here.

use crate::note::{Note, Priority, Tag};
use std::num::NonZeroUsize;

/// Which notes [`crate::Store::list`] returns.
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
