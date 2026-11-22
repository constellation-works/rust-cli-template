---
type: design
summary: "Spec: store — file layout, private modes, atomic replacement, locking, format versioning, bad-record isolation and the audit log"
last_validated: 2026-11-21
---

# Spec: Store

The store never exposes a torn file, never loses a concurrent write, never
rewrites a file a newer build wrote, never silently drops or defaults a bad
record, and never trusts state another user could have written.

## Why This Exists

Two processes adding a note at once (a person and an agent) would otherwise
race a read-modify-write and lose one; a crash mid-write would otherwise
leave unparseable state.

## Layout

- `<root>/` — mode `0700`, created by the first `add`.
- `<root>/notes.json` — `{"format": 1, "notes": [...]}`, mode `0600`.
- `<root>/.lock` — the store write lock file, mode `0600`; created once, never
  deleted; holds the current holder's `pid (label) since <time>`.
- `<root>/.audit.lock` — the audit-log lock file, mode `0600`; separate from
  `.lock` so a writer that timed out on `.lock` still records its failure
  audit line within one deadline.
- `<root>/audit.jsonl` — the audit log, mode `0600`, created by the first
  audited command: one line per mutating command, `{"format": 2, "at",
  "actor", "command", "status", "target", "error_code", "duration_ms"}`.
  `actor` is the OS user name (`USER`, else `USERNAME`, else `null`); no
  sentinel string is used.

## Invariants

- Writes replace `notes.json` by temp file in the same directory, `fsync`,
  rename, `fsync` of the directory. It is never truncated in place.
- Writers hold the `.lock` advisory lock from the read to the rename and
  re-read under it. Readers take no lock.
- Lock acquisition gives up after 10 seconds with `store_locked`, naming the
  holder while that PID is still alive (otherwise reporting an unknown
  process).
- A missing `notes.json` is an empty store. Reads take no lock, create
  nothing and work on a read-only directory.
- Every load refuses a data directory or `notes.json` owned by another user
  (`store_foreign_owner`) or writable by group or others
  (`store_permissions`, naming the `chmod`), and a `notes.json` or lock file
  that is a symbolic link (`store_symlink`). Nothing is repaired
  automatically. The data directory itself may be a link.
- `add` runs every check before its first write, so a refused store gains no
  directory, lock file or temp file.
- `format` greater than this build's is refused (`store_too_new`) for reads
  and writes; the file is left untouched. Format 0 is `store_corrupt`.
- Every change to the persisted shape bumps `format` and appends one step
  to `UPGRADES` (`store/format.rs` for notes, `store/audit.rs` for audit
  lines); shipped steps never change. Reads upgrade an older document or line
  in memory; the next write persists the current format. Format-1 audit lines
  without `actor` upgrade with `"actor": null`.
- Duplicate note ids in `notes.json` are refused on load (`store_duplicate_id`).
  A store already holding id `u64::MAX` refuses `add` (`note_id_exhausted`).
- Each stored note is validated at the construction boundary without silent
  defaults. A note whose title or tag fails validation, or which lacks a
  required field (`id`, `title`, `body`, `tags`, `priority`, `created_at`), is
  isolated as a typed `BadRecord` naming its id (or 1-based line number when
  the id is unreadable) and reason. Reads return the valid notes plus one
  stderr warning per bad record; `show <bad id>` fails with `store_bad_record`;
  writes refuse with `store_bad_record` without touching `notes.json`.
- Encoding failures fail with `encode_error` and never fabricate a filesystem
  path.
- Audit lines are appended in one write under `.audit.lock` and `fsync`ed;
  they are never rewritten. Read-only commands never write one. A line holds
  no argument values or error text. A line that cannot be written is a warning
  (written after the command's payload or JSON error object) and never changes
  the command's result.
