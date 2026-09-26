//! The persisted store document and its format versions.
//!
//! Bytes in, a document out, and back: no I/O (the paths below only name the
//! file in an error). Kept apart from the facade so the format rules
//! (STD-02 R16, STD-03 R10, R23) read in one place and are tested without a
//! filesystem.

use crate::error::{Error, Result};
use crate::note::Note;
use serde::{Deserialize, Serialize};
use std::io;
use std::num::NonZeroU32;
use std::path::Path;

/// The newest store format: this build reads formats 1 to `FORMAT` and
/// writes `FORMAT`.
pub(super) const FORMAT: u32 = 1;

/// One upgrade step: a format `n` document in, the format `n + 1` document out.
pub(super) type Upgrade = fn(serde_json::Value) -> serde_json::Result<serde_json::Value>;

/// The upgrade registry, append-only (STD-03 R23). `UPGRADES[i]` turns format
/// `i + 1` into format `i + 2`, so `FORMAT` is always `UPGRADES.len() + 1` (a
/// unit test checks). Every change to the persisted shape bumps `FORMAT` and
/// appends a step here, so an older build refuses the new format instead of
/// dropping fields it does not know (STD-02 R16, STD-03 R10). A shipped step
/// is never edited, reordered or removed. Reads upgrade in memory only; the
/// next write persists the current format.
pub(super) const UPGRADES: &[Upgrade] = &[];

/// The persisted document. Separate from any output shape: the store format
/// and the `--json` contract evolve independently (STD-02 R16).
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct StoreDocument {
    pub(super) format: u32,
    #[serde(default)]
    pub(super) notes: Vec<Note>,
}

impl StoreDocument {
    /// What a store that does not exist yet reads as.
    pub(super) fn empty() -> Self {
        Self {
            format: FORMAT,
            notes: Vec::new(),
        }
    }
}

/// Just the format number, read before the full document so a newer store is
/// refused by version, not by whatever parse error its new shape causes.
/// Format 0 never existed, so it reads as corrupt.
#[derive(Deserialize)]
struct FormatProbe {
    format: NonZeroU32,
}

/// Parse the store file's bytes, upgrading an older format in memory. A newer
/// format is refused, never reinterpreted (STD-03 R10).
pub(super) fn decode(path: &Path, bytes: &[u8]) -> Result<StoreDocument> {
    let corrupt = |source| Error::Corrupt {
        path: path.to_path_buf(),
        source,
    };
    let mut doc: serde_json::Value = serde_json::from_slice(bytes).map_err(corrupt)?;
    let from = FormatProbe::deserialize(&doc)
        .map_err(corrupt)?
        .format
        .get();
    if from > FORMAT {
        return Err(Error::NewerFormat {
            path: path.to_path_buf(),
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

/// Serialize a document at the current format.
pub(super) fn encode(path: &Path, mut doc: StoreDocument) -> Result<Vec<u8>> {
    doc.format = FORMAT;
    // Serializing plain structs cannot fail in practice; the error path
    // exists so there is no `expect` here (STD-02 R13).
    serde_json::to_vec_pretty(&doc).map_err(|e| Error::io("encode", path, io::Error::other(e)))
}
