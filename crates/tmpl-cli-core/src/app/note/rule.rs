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
use tmpl_cli_store::contract::{Body, Note, NoteId, Priority, Tag, Title};

/// Maximum number of matching notes returned by default when neither
/// `--limit` nor `--all` is supplied (STD-08 R3, R5).
pub const DEFAULT_LIMIT: usize = 20;

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

/// How many matching notes a listing returns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Limit {
    /// At most [`DEFAULT_LIMIT`] matching notes.
    #[default]
    Default,
    /// At most this many matching notes.
    Count(NonZeroUsize),
    /// Every matching note.
    All,
}

/// Which notes a `note list` returns.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListFilter {
    /// Only notes carrying this tag.
    pub tag: Option<Tag>,
    /// Only notes at this priority.
    pub priority: Option<Priority>,
    /// How many of the matching notes to return (oldest first).
    pub limit: Limit,
}

impl ListFilter {
    fn matches(&self, note: &Note) -> bool {
        self.tag.as_ref().is_none_or(|tag| note.tags.contains(tag))
            && self
                .priority
                .is_none_or(|priority| note.priority == priority)
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

/// The id to assign to a note added after `notes`: one past the highest, or
/// [`NoteId::FIRST`] when there are none, or `None` when the highest id is
/// already [`NoteId::MAX`], so an existing id is never handed out again.
pub(crate) fn next_id(notes: &[Note]) -> Option<NoteId> {
    match notes.iter().map(|note| note.id).max() {
        None => Some(NoteId::FIRST),
        Some(max) => max
            .get()
            .checked_add(1)
            .and_then(|next| NoteId::try_from(next).ok()),
    }
}

/// Apply `filter` to `notes`, kept in their stored (oldest-first) order. The
/// filters apply before the limit, and `total` counts every match, so a cut
/// list can say how much it left out (STD-01 R33, R34).
pub(crate) fn select(notes: Vec<Note>, filter: &ListFilter) -> NoteList {
    let mut notes: Vec<Note> = notes
        .into_iter()
        .filter(|note| filter.matches(note))
        .collect();
    let total = notes.len();
    let cap = match filter.limit {
        Limit::Default => Some(DEFAULT_LIMIT),
        Limit::Count(limit) => Some(limit.get()),
        Limit::All => None,
    };
    if let Some(limit) = cap {
        notes.truncate(limit);
    }
    NoteList { notes, total }
}
