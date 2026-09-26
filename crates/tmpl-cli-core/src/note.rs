//! Note contract types.
//!
//! Pure values with validating constructors (STD-02 R14): once a [`Title`] or
//! [`Tag`] exists it is known to be valid, so no caller re-checks it. This
//! module performs no I/O (STD-02 R5); `scripts/check-dependency-direction.sh`
//! rejects `std::fs`, `std::net` and `std::process` here.
//!
//! The serde shapes double as the persisted format (STD-02 R16): never rename
//! or retype a field, and give a new one a default. Any change to the shape
//! also bumps the store format with an upgrade step (`store.rs::UPGRADES`).

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use time::OffsetDateTime;

/// Why a value was rejected as a note field.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NoteError {
    /// The id was not a positive integer.
    #[error("invalid note id '{0}': expected a positive integer such as 1")]
    InvalidId(String),
    /// The title was empty or only whitespace.
    #[error("title is empty: pass some text, e.g. \"Buy milk\"")]
    EmptyTitle,
    /// The title was longer than [`Title::MAX_CHARS`].
    #[error("title is {0} characters; the limit is {max}", max = Title::MAX_CHARS)]
    TitleTooLong(usize),
    /// The title contained a tab or line break.
    #[error("title contains a tab or line break; titles are a single line")]
    TitleNotSingleLine,
    /// The body was empty or only whitespace.
    #[error("body is blank: give it some text, or leave the body out")]
    EmptyBody,
    /// The tag had characters outside `a-z`, `0-9` and `-`, or a bad length.
    #[error(
        "invalid tag '{0}': use 1-{max} lowercase letters, digits or '-'",
        max = Tag::MAX_CHARS
    )]
    InvalidTag(String),
    /// The priority was not one of [`Priority::NAMES`].
    #[error("invalid priority '{0}': expected one of low, normal, high")]
    InvalidPriority(String),
}

/// A note's identifier: a positive integer the store assigns on add.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct NoteId(u64);

impl NoteId {
    /// The first id a store hands out.
    pub const FIRST: Self = Self(1);

    /// The id as a number.
    #[must_use]
    pub fn get(self) -> u64 {
        self.0
    }

    /// The id after this one. Saturates rather than wrapping: a store would
    /// need 2^64 notes to reach it.
    pub(crate) fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl TryFrom<u64> for NoteId {
    type Error = NoteError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        if value == 0 {
            return Err(NoteError::InvalidId(value.to_string()));
        }
        Ok(Self(value))
    }
}

impl From<NoteId> for u64 {
    fn from(id: NoteId) -> Self {
        id.0
    }
}

impl FromStr for NoteId {
    type Err = NoteError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value: u64 = s
            .trim()
            .parse()
            .map_err(|_| NoteError::InvalidId(s.to_owned()))?;
        Self::try_from(value).map_err(|_| NoteError::InvalidId(s.to_owned()))
    }
}

impl fmt::Display for NoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A note's title: one non-empty line, trimmed.
///
/// Single-line is load-bearing, not cosmetic: the piped output form is one
/// tab-separated line per record (STD-01 R9), and a title with a tab or a
/// newline would split a record across fields or lines.
///
/// ```
/// use tmpl_cli_core::Title;
///
/// let title: Title = "  Buy milk ".parse()?;
/// assert_eq!(title.as_str(), "Buy milk");
/// assert!("Buy\tmilk".parse::<Title>().is_err(), "a title is one line");
/// # Ok::<(), tmpl_cli_core::NoteError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Title(String);

impl Title {
    /// The longest title accepted, in characters.
    pub const MAX_CHARS: usize = 200;

    /// The title text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Title {
    type Error = NoteError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(NoteError::EmptyTitle);
        }
        if trimmed.contains(['\t', '\n', '\r']) {
            return Err(NoteError::TitleNotSingleLine);
        }
        let chars = trimmed.chars().count();
        if chars > Self::MAX_CHARS {
            return Err(NoteError::TitleTooLong(chars));
        }
        Ok(Self(trimmed.to_owned()))
    }
}

impl From<Title> for String {
    fn from(title: Title) -> Self {
        title.0
    }
}

impl FromStr for Title {
    type Err = NoteError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s.to_owned())
    }
}

/// A note's body as a caller supplies it: any text, possibly several lines,
/// but never blank.
///
/// Blank text is refused rather than quietly stored as no body (STD-01 R29).
/// Only new input goes through this type; the persisted [`Note::body`] stays a
/// plain string, so stored notes keep loading unchanged (STD-03 R24).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Body(String);

impl Body {
    /// The body text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Body {
    type Err = NoteError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.trim().is_empty() {
            return Err(NoteError::EmptyBody);
        }
        Ok(Self(s.to_owned()))
    }
}

impl From<Body> for String {
    fn from(body: Body) -> Self {
        body.0
    }
}

/// A tag: a short lowercase label such as `home` or `follow-up`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Tag(String);

impl Tag {
    /// The longest tag accepted, in characters.
    pub const MAX_CHARS: usize = 32;

    /// The tag text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Tag {
    type Error = NoteError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let valid = !value.is_empty()
            && value.len() <= Self::MAX_CHARS
            && value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if valid {
            Ok(Self(value))
        } else {
            Err(NoteError::InvalidTag(value))
        }
    }
}

impl From<Tag> for String {
    fn from(tag: Tag) -> Self {
        tag.0
    }
}

impl FromStr for Tag {
    type Err = NoteError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s.to_owned())
    }
}

/// How much a note matters. Serialized as its lowercase name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    /// Can wait.
    Low,
    /// The default.
    #[default]
    Normal,
    /// Needs attention first.
    High,
}

impl Priority {
    /// Every priority's canonical token, in ascending order. Surfaces list
    /// allowed values from here rather than repeating them (STD-01 R25).
    pub const NAMES: [&'static str; 3] = ["low", "normal", "high"];

    /// The canonical lowercase token.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
        }
    }
}

impl FromStr for Priority {
    type Err = NoteError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "low" => Ok(Self::Low),
            "normal" => Ok(Self::Normal),
            "high" => Ok(Self::High),
            other => Err(NoteError::InvalidPriority(other.to_owned())),
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A stored note.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    /// Assigned by the store; unique within it.
    pub id: NoteId,
    /// One line of text.
    pub title: Title,
    /// Optional longer text.
    #[serde(default)]
    pub body: Option<String>,
    /// Labels, sorted and deduplicated.
    #[serde(default)]
    pub tags: Vec<Tag>,
    /// How much it matters.
    #[serde(default)]
    pub priority: Priority,
    /// When it was added.
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// The fields a caller supplies to add a note; the store assigns the rest.
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
