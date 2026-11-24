//! The persisted store document and its format versions.
//!
//! Bytes in, a document out, and back: no I/O (the paths below only name the
//! file in an error). Kept apart from the store facade so the format rules
//! (STD-02 R16, R32, STD-03 R10, R23) read in one place and are tested without
//! a filesystem.

use crate::contracts::{Note, NoteId, Priority, Tag, Title};
use crate::error::{BadRecord, BadRecordReason, Error, RecordLocator, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::num::NonZeroU32;
use std::path::Path;
use std::str::FromStr;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

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

/// The decoded store document: valid notes plus any isolated bad records.
#[derive(Debug)]
pub(crate) struct StoreDocument {
    pub(crate) format: u32,
    pub(crate) notes: Vec<Note>,
    pub(crate) bad_records: Vec<BadRecord>,
}

impl StoreDocument {
    /// What a store that does not exist yet reads as.
    pub(crate) fn empty() -> Self {
        Self {
            format: FORMAT,
            notes: Vec::new(),
            bad_records: Vec::new(),
        }
    }
}

/// The serialized form written to disk. Separate from any output shape: the
/// store format and the `--json` contract evolve independently (STD-02 R16).
#[derive(Serialize)]
struct EncodedDocument<'a> {
    format: u32,
    notes: &'a [Note],
}

/// Just the format number, read before the full document so a newer store is
/// refused by version, not by whatever parse error its new shape causes.
/// Format 0 never existed, so it reads as corrupt.
#[derive(Deserialize)]
struct FormatProbe {
    format: NonZeroU32,
}

/// Envelope shape verified after format upgrades: `notes` must be an array.
#[derive(Deserialize)]
struct Envelope {
    notes: Vec<serde_json::Value>,
}

/// Parse the store file's bytes, upgrading an older format in memory. A newer
/// format is refused, never reinterpreted (STD-03 R10). Individual notes are
/// validated at the construction boundary so one bad record is isolated and
/// reported rather than making the whole store unreadable (STD-02 R32).
/// Duplicate note ids are refused as a store-level integrity error.
pub(crate) fn decode(path: &Path, bytes: &[u8]) -> Result<StoreDocument> {
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
    let envelope: Envelope = serde_json::from_value(doc).map_err(corrupt)?;
    let entry_lines = note_entry_lines(bytes);

    let mut notes = Vec::with_capacity(envelope.notes.len());
    let mut bad_records = Vec::new();
    let mut seen_ids = HashSet::with_capacity(envelope.notes.len());

    for (idx, raw) in envelope.notes.into_iter().enumerate() {
        let line = entry_lines.get(idx).copied().unwrap_or(1);
        match decode_note(path, raw, line, &mut seen_ids)? {
            Ok(note) => notes.push(note),
            Err(bad) => bad_records.push(bad),
        }
    }

    Ok(StoreDocument {
        format: FORMAT,
        notes,
        bad_records,
    })
}

/// Decode one note entry, validating every required field at the construction
/// boundary. Returns `Err(Error::DuplicateId)` for a repeated id, `Ok(Err(bad))`
/// for an isolated bad record, and `Ok(Ok(note))` for a valid note.
fn decode_note(
    path: &Path,
    raw: serde_json::Value,
    line: usize,
    seen_ids: &mut HashSet<NoteId>,
) -> Result<std::result::Result<Note, BadRecord>> {
    let make_bad = |locator, reason| BadRecord {
        path: path.to_path_buf(),
        locator,
        reason,
    };
    let Some(obj) = raw.as_object() else {
        return Ok(Err(make_bad(
            RecordLocator::Line(line),
            BadRecordReason::NotAnObject,
        )));
    };

    let id = match parse_id(obj) {
        Ok(id) => id,
        Err(reason) => return Ok(Err(make_bad(RecordLocator::Line(line), reason))),
    };
    if !seen_ids.insert(id) {
        return Err(Error::DuplicateId {
            path: path.to_path_buf(),
            id,
        });
    }

    let locator = RecordLocator::Id(id);
    let title = match parse_title(obj) {
        Ok(title) => title,
        Err(reason) => return Ok(Err(make_bad(locator, reason))),
    };
    let body = match parse_body(obj) {
        Ok(body) => body,
        Err(reason) => return Ok(Err(make_bad(locator, reason))),
    };
    let tags = match parse_tags(obj) {
        Ok(tags) => tags,
        Err(reason) => return Ok(Err(make_bad(locator, reason))),
    };
    let priority = match parse_priority(obj) {
        Ok(priority) => priority,
        Err(reason) => return Ok(Err(make_bad(locator, reason))),
    };
    let created_at = match parse_created_at(obj) {
        Ok(created_at) => created_at,
        Err(reason) => return Ok(Err(make_bad(locator, reason))),
    };

    Ok(Ok(Note {
        id,
        title,
        body,
        tags,
        priority,
        created_at,
    }))
}

fn parse_id(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> std::result::Result<NoteId, BadRecordReason> {
    let value = obj.get("id").ok_or(BadRecordReason::MissingField("id"))?;
    let raw = value
        .as_u64()
        .ok_or_else(|| BadRecordReason::InvalidField {
            field: "id",
            detail: String::from("expected a positive integer"),
        })?;
    NoteId::try_from(raw).map_err(BadRecordReason::Validation)
}

fn parse_title(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> std::result::Result<Title, BadRecordReason> {
    let value = obj
        .get("title")
        .ok_or(BadRecordReason::MissingField("title"))?;
    let text = value
        .as_str()
        .ok_or_else(|| BadRecordReason::InvalidField {
            field: "title",
            detail: String::from("expected a string"),
        })?;
    Title::try_from(text.to_owned()).map_err(BadRecordReason::Validation)
}

fn parse_body(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> std::result::Result<Option<String>, BadRecordReason> {
    let value = obj
        .get("body")
        .ok_or(BadRecordReason::MissingField("body"))?;
    match value {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::String(s) => Ok(Some(s.clone())),
        serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::Array(_)
        | serde_json::Value::Object(_) => Err(BadRecordReason::InvalidField {
            field: "body",
            detail: String::from("expected a string or null"),
        }),
    }
}

fn parse_tags(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> std::result::Result<Vec<Tag>, BadRecordReason> {
    let value = obj
        .get("tags")
        .ok_or(BadRecordReason::MissingField("tags"))?;
    let items = value
        .as_array()
        .ok_or_else(|| BadRecordReason::InvalidField {
            field: "tags",
            detail: String::from("expected an array of strings"),
        })?;
    let mut tags = Vec::with_capacity(items.len());
    for item in items {
        let s = item.as_str().ok_or_else(|| BadRecordReason::InvalidField {
            field: "tags",
            detail: String::from("expected an array of strings"),
        })?;
        tags.push(Tag::try_from(s.to_owned()).map_err(BadRecordReason::Validation)?);
    }
    Ok(tags)
}

fn parse_priority(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> std::result::Result<Priority, BadRecordReason> {
    let value = obj
        .get("priority")
        .ok_or(BadRecordReason::MissingField("priority"))?;
    let text = value
        .as_str()
        .ok_or_else(|| BadRecordReason::InvalidField {
            field: "priority",
            detail: String::from("expected a string"),
        })?;
    Priority::from_str(text).map_err(BadRecordReason::Validation)
}

fn parse_created_at(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> std::result::Result<OffsetDateTime, BadRecordReason> {
    let value = obj
        .get("created_at")
        .ok_or(BadRecordReason::MissingField("created_at"))?;
    let text = value
        .as_str()
        .ok_or_else(|| BadRecordReason::InvalidField {
            field: "created_at",
            detail: String::from("expected an RFC 3339 timestamp string"),
        })?;
    OffsetDateTime::parse(text, &Rfc3339).map_err(|e| BadRecordReason::InvalidField {
        field: "created_at",
        detail: e.to_string(),
    })
}

/// Find the 1-based line number where each element of the top-level `"notes"`
/// array starts, so a bad record whose `id` is unreadable still points to its
/// location in the file.
fn note_entry_lines(bytes: &[u8]) -> Vec<usize> {
    let mut lines = Vec::new();
    let mut line = 1usize;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escape = false;
    let mut capture_key = false;
    let mut key = String::new();
    let mut last_key = String::new();
    let mut awaiting_notes = false;
    let mut in_notes = false;
    let mut expect_element = false;

    for &b in bytes {
        if b == b'\n' {
            line += 1;
        }
        if in_string {
            if escape {
                escape = false;
                if capture_key {
                    key.push(char::from(b));
                }
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_string = false;
                if capture_key {
                    last_key = std::mem::take(&mut key);
                    capture_key = false;
                }
            } else if capture_key {
                key.push(char::from(b));
            }
            continue;
        }

        if b.is_ascii_whitespace() {
            continue;
        }

        if in_notes && depth == 2 && expect_element && b != b']' {
            lines.push(line);
            expect_element = false;
        }

        match b {
            b'"' => {
                in_string = true;
                if depth == 1 && !awaiting_notes {
                    capture_key = true;
                    key.clear();
                }
            }
            b':' if depth == 1 => {
                if last_key == "notes" {
                    awaiting_notes = true;
                }
            }
            b'[' => {
                if awaiting_notes && depth == 1 {
                    awaiting_notes = false;
                    in_notes = true;
                    expect_element = true;
                }
                depth += 1;
            }
            b'{' => {
                awaiting_notes = false;
                depth += 1;
            }
            b']' => {
                if in_notes && depth == 2 {
                    in_notes = false;
                    expect_element = false;
                }
                depth = depth.saturating_sub(1);
            }
            b'}' => {
                depth = depth.saturating_sub(1);
            }
            b',' if in_notes && depth == 2 => {
                expect_element = true;
            }
            b',' if depth == 1 => {
                awaiting_notes = false;
                last_key.clear();
            }
            _ => {
                if awaiting_notes && depth == 1 {
                    awaiting_notes = false;
                }
            }
        }
    }
    lines
}

/// Serialize a document at its (current) format.
pub(crate) fn encode(doc: &StoreDocument) -> Result<Vec<u8>> {
    let encoded = EncodedDocument {
        format: doc.format,
        notes: &doc.notes,
    };
    serde_json::to_vec_pretty(&encoded).map_err(|source| Error::Encode {
        what: "store document",
        source,
    })
}
