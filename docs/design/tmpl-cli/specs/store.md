---
type: design
summary: "Spec: store — the notes database, its files and modes, its schema steps, transactions, the record-access seam, bad-record isolation and the audit log"
last_validated: 2026-11-25
---

# Spec: Store

The store never exposes a half-written change, never loses a concurrent
write, never reads or writes a database a newer build migrated, never
silently drops or defaults a bad record, and never trusts state another user
could have written.

## Why This Exists

Two processes adding a note at once (a person and an agent) would otherwise
race a read-modify-write and lose one; a crash mid-write would otherwise
leave unreadable state.

## Layout

- `<root>/` — mode `0700`, created by the first `add`.
- `<root>/store.db` — the notes, a SQLite database in WAL mode, mode `0600`,
  created by the first `add`. SQLite keeps `<root>/store.db-wal` and
  `<root>/store.db-shm` beside it, also `0600`; both stay after the last
  connection closes, so a read-only reader finds them.
- `<root>/.audit.lock` — the audit-log lock file, mode `0600`; created once,
  never deleted. While held it holds one JSON line naming the holder,
  `{"pid", "start" (Linux only), "time", "label"}`, at most 4096 bytes; the
  holder clears it on release. The record only explains a timeout; nothing
  decides ownership from it.
- `<root>/audit.jsonl` — the audit log, mode `0600`, created by the first
  audited command: one line per mutating command, `{"format": 2, "at",
  "actor", "command", "status", "target", "error_code", "duration_ms"}`.
  `actor` is the OS user name (`USER`, else `USERNAME`, else `null`); no
  sentinel string is used.

## Schema

The schema is the driver's `MIGRATIONS` in
`crates/tmpl-cli-store/src/driver/sqlite/schema/mod.rs`, an append-only list
of steps keyed on `PRAGMA user_version`: step `n` takes the database from
version `n - 1` to `n`. Each table's steps are named items in its own
module, `schema/<table>.rs` (`schema/note.rs` for the note table), and the
list names each one in order. A shipped step is never edited, reordered or
removed; a change is a new step at the end of the list, even for an earlier
table, so the list is never built by concatenating per-table lists. A
statement that changes the schema appears only under `schema/`
(`scripts/check-dependency-direction.sh`).

Version 1 is one `STRICT` table:

| Column | Type | Holds |
|--------|------|-------|
| `id` | `INTEGER PRIMARY KEY`, `> 0` | the note id, `1` to `9223372036854775807` (`NoteId::MAX`) |
| `title` | `TEXT NOT NULL` | the title |
| `body` | `TEXT`, nullable | the body, or `NULL` |
| `tags` | `TEXT NOT NULL`, a JSON array (`json_valid`) | the tags, sorted |
| `priority` | `TEXT NOT NULL` | `low`, `normal` or `high` |
| `created_at` | `TEXT NOT NULL` | RFC 3339, UTC |

## Invariants

- The kit's `cw_sqlite` is the mechanism: WAL, `synchronous = FULL`,
  `foreign_keys` on, no link followed, the database created `0600` before
  SQLite opens it, and the database and its `-wal` and `-shm` refused when
  they are a link, another user's, or open to any group or other permission
  bit (`store_symlink`, `store_foreign_owner`, `store_permissions`, naming the
  `chmod -- 600 '<path>'` that fixes it).
- The data directory is checked the same way on every read and write before
  anything is opened or created in it; a refused root gains nothing. The data
  directory itself may not be a link; a link in a component above it is used
  as given.
- A write is one `IMMEDIATE` transaction (`Store::write`): it commits when the
  caller's closure succeeds and rolls back when it fails, so no partial change
  is ever visible. Two writers never interleave: the second waits.
- A writer waits up to 10 seconds for another connection, then refuses with
  `store_locked`. SQLite's locks record no owner, so the refusal names no
  holder.
- A read is one read-only connection holding one read transaction
  (`Store::read`), so every query in it sees one committed version. It takes
  no lock and writes no row. In a directory this process can write, the
  reader shares the `-shm` index with a writer in the same process, so a
  snapshot held across `Store::write` does not block that write; SQLite may
  rebuild the `-shm` (its index, not note content) and creates a missing
  `-wal` (empty) or `-shm`, owner-only. In a directory this process cannot
  write, a read writes nothing: the index is opened read-only, and when the
  WAL cannot be read that way the database is read `immutable` if its `-wal`
  is empty and refused otherwise.
- A missing data directory or database is an empty store: a read creates
  nothing.
- A database at a newer schema version than this build's is refused for reads
  and writes (`store_too_new`) and left as it is. A database at an older
  version is migrated by the next write; each step runs in its own
  transaction.
- A file that is not a SQLite database, or one SQLite finds damaged, is
  `store_corrupt`.
- Nothing is repaired automatically.
- `add` runs every check before its first write, so a refused store gains no
  directory and no database.
- Each row is validated at the construction boundary without silent defaults.
  A row whose title, tag or priority fails validation, whose tags are not an
  array of strings, or whose `created_at` is not RFC 3339 is isolated as a
  typed `BadRecord` naming its id and reason. Reads return the valid notes plus
  one stderr warning per bad record; `show <bad id>` fails with
  `store_bad_record`; writes refuse with `store_bad_record` and change nothing.
- A store already holding id `NoteId::MAX` refuses `add` (`note_id_exhausted`).
- Encoding failures fail with `encode_error` and never fabricate a filesystem
  path.
- Audit lines are appended in one write under `.audit.lock` and `fsync`ed;
  they are never rewritten. Read-only commands never write one. A line holds
  no argument values or error text. A line that cannot be written is a warning
  (written after the command's payload or JSON error object) and never changes
  the command's result. Each line carries its own `format`; a reader upgrades
  older lines through `UPGRADES` in `repository/audit/line.rs` (format-1 lines without
  `actor` upgrade with `"actor": null`) and refuses newer ones.

## Record access

The core reaches a record only through `snapshot.<record>()` (a read) and
`tx.<record>()` (a write); today that is `notes()`. The store's public
surface carries no `rusqlite` or `cw_sqlite` type, no borrow from a
connection, and no path but `Store::path()` (the note app names the database
in its outcome and refusals) and `Store::audit_path()` (the core's audit
guard names the log in its warning). Repository calls return owned values.

## Error codes

| Code | Kind | When |
|------|------|------|
| `store_locked` | busy | another connection held the database past the lock wait, or another append held `.audit.lock` |
| `store_corrupt` | corrupt | not a SQLite database, or damaged; an unparseable audit line |
| `store_check_failed` | corrupt | an integrity check reported problems (no command runs one yet) |
| `store_too_new` | incompatible | a newer schema version, or a newer audit-line format |
| `store_bad_record` | corrupt | a note row failed validation |
| `store_symlink`, `store_foreign_owner`, `store_permissions`, `store_not_regular` | refused | a file or the data directory failed the owner-only checks |
| `store_too_large`, `store_lock_reentrant` | as the kit classifies them | the kit's other file refusals; the store keeps a code for each, though no current path meets them |
| `store_missing` | not found | the database does not exist (a read treats it as empty, so no command reports it) |
| `store_not_wal` | io | the database could not be put in WAL mode |
| `store_migration_failed` | io | a schema step failed; the schema and version are as they were |
| `store_sqlite_error`, `io_error` | io | any other SQLite or file failure |
| `encode_error` | internal | a note's tags or timestamp, or an audit event, could not be encoded |
