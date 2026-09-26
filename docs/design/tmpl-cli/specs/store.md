---
type: design
summary: "Spec: store — file layout, private modes, atomic replacement, locking, format versioning and the audit log"
last_validated: 2026-09-26
---

# Spec: Store

The store never exposes a torn file, never loses a concurrent write, never
rewrites a file a newer build wrote, and never trusts state another user
could have written.

## Why This Exists

Two processes adding a note at once (a person and an agent) would otherwise
race a read-modify-write and lose one; a crash mid-write would otherwise
leave unparseable state.

## Layout

- `<root>/` — mode `0700`, created by the first `add`.
- `<root>/notes.json` — `{"format": 1, "notes": [...]}`, mode `0600`.
- `<root>/.lock` — the lock file, mode `0600`; created once, never deleted;
  holds the current holder's `pid (label) since <time>`.
- `<root>/audit.jsonl` — the audit log, mode `0600`, created by the first
  audited command: one line per mutating command, `{"format": 1, "at",
  "command", "status", "target", "error_code", "duration_ms"}`.

## Invariants

- Writes replace `notes.json` by temp file in the same directory, `fsync`,
  rename, `fsync` of the directory. It is never truncated in place.
- Writers hold the `.lock` advisory lock from the read to the rename and
  re-read under it. Readers take no lock.
- Lock acquisition gives up after 10 seconds with `store_locked`, naming the
  holder.
- A missing `notes.json` is an empty store. Reads take no lock, create
  nothing and work on a read-only directory.
- Every load refuses a data directory or `notes.json` owned by another user
  (`store_foreign_owner`) or writable by group or others
  (`store_permissions`, naming the `chmod`), and a `notes.json` or `.lock`
  that is a symbolic link (`store_symlink`). Nothing is repaired
  automatically. The data directory itself may be a link.
- `add` runs every check before its first write, so a refused store gains no
  directory, lock file or temp file.
- `format` greater than this build's is refused (`store_too_new`) for reads
  and writes; the file is left untouched. Format 0 is `store_corrupt`.
- Every change to the persisted shape bumps `format` and appends one step
  to `UPGRADES` in `store/format.rs`; shipped steps never change. Reads upgrade an
  older document in memory; the next write persists the current format.
- One unparseable note makes the whole file `store_corrupt`.
- Audit lines are appended in one write under `.lock` and `fsync`ed; they are
  never rewritten. Read-only commands never write one. A line holds no
  argument values or error text. A line that cannot be written is a warning
  and never changes the command's result.
