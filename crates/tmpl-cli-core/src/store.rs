//! The file-backed note store.
//!
//! One JSON document, `<root>/notes.json`, holds every note. Reads take no
//! lock and write nothing (STD-01 R31): the file is only ever replaced by an
//! atomic rename, so a reader sees a whole old version or a whole new one.
//! Writes are a read-modify-write, so they hold `<root>/.lock` from the read
//! to the rename; two concurrent `add`s therefore never assign the same id or
//! lose a note (STD-03 R5, R6). The directory is `0700` and the files `0600`,
//! and a load refuses state that is not (STD-05 R8, R9).

use crate::error::{Error, Result};
use crate::fsio::{self, LockGuard, write_atomic};
use crate::note::{NewNote, Note, NoteId, Priority, Tag};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::num::{NonZeroU32, NonZeroUsize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use time::OffsetDateTime;

/// The store file's name under the data directory.
pub const STORE_FILE: &str = "notes.json";

/// The lock file's name under the data directory.
const LOCK_FILE: &str = ".lock";

/// The newest store format: this build reads formats 1 to `FORMAT` and
/// writes `FORMAT`.
pub(crate) const FORMAT: u32 = 1;

/// One upgrade step: a format `n` document in, the format `n + 1` document out.
pub(crate) type Upgrade = fn(serde_json::Value) -> serde_json::Result<serde_json::Value>;

/// The upgrade registry, append-only (STD-03 R23). `UPGRADES[i]` turns format
/// `i + 1` into format `i + 2`, so `FORMAT` is always `UPGRADES.len() + 1` (a
/// unit test checks). Every change to the persisted shape bumps `FORMAT` and
/// appends a step here, so an older build refuses the new format instead of
/// dropping fields it does not know (STD-02 R16, STD-03 R10). A shipped step
/// is never edited, reordered or removed. Reads upgrade in memory only; the
/// next write persists the current format.
pub(crate) const UPGRADES: &[Upgrade] = &[];

/// How long a writer waits for another writer before refusing (STD-03 R7).
pub const DEFAULT_LOCK_WAIT: Duration = Duration::from_secs(10);

/// The persisted document. Separate from any output shape: the store format
/// and the `--json` contract evolve independently (STD-02 R16).
#[derive(Debug, Serialize, Deserialize)]
struct StoreDocument {
    format: u32,
    #[serde(default)]
    notes: Vec<Note>,
}

/// Just the format number, read before the full document so a newer store is
/// refused by version, not by whatever parse error its new shape causes.
/// Format 0 never existed, so it reads as corrupt.
#[derive(Deserialize)]
struct FormatProbe {
    format: NonZeroU32,
}

/// Which notes [`Store::list`] returns.
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

/// What [`Store::list`] found: the notes returned and how many matched.
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

/// A note store rooted at one data directory.
#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
    lock_wait: Duration,
}

impl Store {
    /// A store at `root`. Performs no I/O; the directory is created by the
    /// first write. The caller resolves `root` (STD-02 R3): this crate never
    /// reads the environment or the home directory.
    pub fn open(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            lock_wait: DEFAULT_LOCK_WAIT,
        }
    }

    /// Override how long a writer waits for the lock.
    #[must_use]
    pub fn with_lock_wait(mut self, wait: Duration) -> Self {
        self.lock_wait = wait;
        self
    }

    /// The data directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The store file, `<root>/notes.json`.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.root.join(STORE_FILE)
    }

    /// Notes matching `filter`, oldest first, and how many matched. The
    /// filters apply before the limit (STD-01 R33). A missing store is an
    /// empty one.
    pub fn list(&self, filter: &ListFilter) -> Result<NoteList> {
        let mut notes: Vec<Note> = self
            .load()?
            .notes
            .into_iter()
            .filter(|n| filter.matches(n))
            .collect();
        let total = notes.len();
        if let Some(limit) = filter.limit {
            notes.truncate(limit.get());
        }
        Ok(NoteList { notes, total })
    }

    /// The note with this id.
    pub fn show(&self, id: NoteId) -> Result<Note> {
        self.load()?
            .notes
            .into_iter()
            .find(|n| n.id == id)
            .ok_or_else(|| Error::NotFound {
                id,
                path: self.path(),
            })
    }

    /// Add a note, assigning the next id. `now` is the creation time; the
    /// caller supplies it so tests can pin it.
    pub fn add(&self, new: NewNote, now: OffsetDateTime) -> Result<Note> {
        // Every check that can refuse runs before the first write (STD-02
        // R34): an unsafe, corrupt or newer store is refused here, before the
        // directory or the lock file is created.
        self.load()?;
        fsio::create_private_dir(&self.root)?;
        let _lock = LockGuard::acquire(&self.root.join(LOCK_FILE), "note add", self.lock_wait)?;
        // Re-read under the lock: anything read before it may be stale.
        let mut doc = self.load()?;
        let id = doc
            .notes
            .iter()
            .map(|n| n.id)
            .max()
            .map_or(NoteId::FIRST, NoteId::next);
        let note = new.into_note(id, now);
        doc.notes.push(note.clone());
        doc.format = FORMAT;
        let path = self.path();
        // Serializing plain structs cannot fail in practice; the error path
        // exists so there is no `expect` here (STD-02 R13).
        let bytes = serde_json::to_vec_pretty(&doc)
            .map_err(|e| Error::io("encode", path.clone(), io::Error::other(e)))?;
        write_atomic(&path, &bytes)?;
        tracing::debug!(id = %note.id, path = %path.display(), "added note");
        Ok(note)
    }

    fn load(&self) -> Result<StoreDocument> {
        let path = self.path();
        if !fsio::inspect_dir(&self.root)? || !fsio::inspect_file(&path)? {
            return Ok(StoreDocument {
                format: FORMAT,
                notes: Vec::new(),
            });
        }
        let bytes = fs::read(&path).map_err(|e| Error::io("read", &path, e))?;
        let corrupt = |source| Error::Corrupt {
            path: path.clone(),
            source,
        };
        let mut doc: serde_json::Value = serde_json::from_slice(&bytes).map_err(corrupt)?;
        let from = FormatProbe::deserialize(&doc)
            .map_err(corrupt)?
            .format
            .get();
        if from > FORMAT {
            return Err(Error::NewerFormat {
                path,
                found: from,
                supported: FORMAT,
            });
        }
        // Step `i` produces format `i + 2`; apply the ones past `from`.
        for (step, to) in UPGRADES.iter().zip(2..) {
            if to > from {
                doc = step(doc).map_err(corrupt)?;
            }
        }
        serde_json::from_value(doc).map_err(corrupt)
    }
}
