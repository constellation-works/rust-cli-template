---
title: tmpl-cli — Design
owner: claude
last_updated: 2026-11-24
last_validated: 2026-09-26
status: Draft
feature: tmpl-cli
doc_role: design
type: design
summary: How tmpl-cli is layered, how an invocation flows from argv to bytes, how the store stays consistent, what the audit log records, and which gates hold each standard in place.
tags: [tmpl-cli, cli, architecture]
paths: ["crates/**", "scripts/**", "Makefile"]
related_features: [tmpl-cli]
---

# tmpl-cli — Design

The current implementation: layering, the life of one invocation, the store,
and the gates. The output and store contracts are specified in
[specs/output-contract.md](./specs/output-contract.md) and
[specs/store.md](./specs/store.md); open directions are in
[3_vision.md](./3_vision.md).

## 1. Layering

Three crates laid out by app over the cw-kit (why:
[4_decisions.md](./4_decisions.md#the-template-is-laid-out-by-app-in-three-crates),
[the kit](./4_decisions.md#the-foundation-is-the-cw-kit)).
`tmpl-cli` is the surface: `cli` (the root flags), `command` (one module
per noun, holding its subcommands, flags and help, and mapping its arguments
to an app's `Command` and the `Outcome` to a payload) and `output`.
`tmpl-cli-core` holds the apps (`app/note/`: `Command`, `Outcome`, `Error`,
`exec`, one flow per command and the pure `rule`: the next id and list
selection), the `runtime` they share (the context, the one chokepoint every
app's `exec` forwards to, the audit guard) and `composition` (the only code
that reads the environment or picks the data directory, the store and the
clock). `tmpl-cli-store` owns
the data directory: the `Store` facade (one read, one write transaction)
over `driver/sqlite/` (the notes database's file and lock wait, and in
`schema/` one module per table and the one ordered step list, handed out as
a `cw_sqlite::Db`; no query), one repository per record
(`note`: the notes' SQL and row mapping; `audit`), `db/audit.rs` (the
persisted audit line) and the record shapes in `contract/`. A driver sets
one storage mechanism up once and returns a handle; a repository is one
record's use of it. The core reaches a record only through
`snapshot.<record>()` and `tx.<record>()`. The foundation is the cw-kit from
[constellation-works/satellites](https://github.com/constellation-works/satellites),
pinned to commit `9c9cc998720f0a75041be3451c14fbaf80f29dc8`: `cw-base` (the
one error type every unit translates its typed error into, with its `Kind`
and stable code), which the store and the core name; `cw-fs` (appends,
locking and private-state checks, mechanism only) and `cw-sqlite` (one
private SQLite database, mechanism only, re-exporting `rusqlite`), which
only the store names. No library has clap or terminal access. The crate and module rules are in
[ARCHITECTURE.md](../../../ARCHITECTURE.md), and
`scripts/check-dependency-direction.sh` enforces them from source text
alone, so it runs in `make ci-fast` without compiling.

## 2. The life of an invocation

1. `main` parses argv with `Cli::parse_args`. `--help`/`--version` go to
   stdout (exit 0); a usage error goes to stderr (exit 2) — as one JSON
   object when the raw arguments or `TMPL_CLI_FORMAT` asked for JSON.
2. `Sink::from_process` resolves the output mode (flag > `TMPL_CLI_FORMAT`
   > auto), color (`NO_COLOR`, `CLICOLOR_FORCE`, `TERM`, TTY) and width
   (`COLUMNS`, terminals only). Nothing else in the program asks these
   questions; `scripts/check-terminal-guard.sh` rejects any attempt.
3. The noun's handler maps the parsed arguments to the app's `Command` and
   calls `app::note::exec` with the environment `main` captured (`Env`, plus
   `--root`). The app's `exec` forwards to the runtime's `exec`
   (`runtime/exec.rs`), which starts the runtime: composition picks the data
   directory, resolves the OS actor (`USER`, else `USERNAME`, else `None`),
   and opens the store.
4. The runtime's `exec` is the one chokepoint, written once for every app;
   the app supplies its parts through the `App` trait. If the command is
   audited it starts an `AuditGuard` (section 5); then the command's flow loads from the store,
   decides with the app's rules and commits in one write. The handler turns
   the `Outcome` into a payload (`Output::Notes` or `Output::Note`) with an
   optional stderr notice and any bad-record warnings; an app's typed error has already
   crossed into the one error type through the app's translator, inside the
   runtime's `exec`. The guard is told the outcome,
   `output::finish` renders the payload or error inside the `finish` that
   `exec` was given, and then the guard drops and writes its line, so any
   fail-open audit warning reaches stderr after the payload or JSON error
   object.
5. `output::finish` renders the payload for the sink (writing any bad-record
   warnings and notices to stderr in every mode), or reports the error
   (`error: …` line, or `{"error","code"}` in JSON mode) and returns exit 1.
   A write that fails with `BrokenPipe` ends the process with exit 0 and no
   message.

stdout carries only the payload. Notices (nothing matched, the list was cut,
`added note <id> to <file>`), errors and `tracing` logs (`TMPL_CLI_LOG`,
default `warn`; an unparseable filter is reported and ignored) go to stderr.

## 3. Rendering

Every mode renders the same `NotePayload`. `table` is borderless, one header
row, two-space gutters, the ID column right-aligned, absent cells as `-`,
priority colored by role (high → warn, low → muted) when color is allowed.
On a terminal with a known width the TITLE column shrinks and ends in `…`;
`note show` and `--json` always carry the full title. `plain` (auto in a
pipe) is one tab-separated line per record without a header. `json` is one
pretty-printed document; for a list it is always the envelope `{"notes",
"total", "truncated"}`. A single-record list is still a table; an empty list
prints nothing (or the envelope with no notes) plus one stderr line naming
the filters. Filters apply before `--limit`, and a cut list says so on stderr
in every mode.

## 4. The store

The notes live in one SQLite database, `notes.db`, with SQLite's
`notes.db-wal` and `notes.db-shm` beside it, opened through the kit's
`cw_sqlite` (WAL, `synchronous = FULL`, owner-only files, no link followed).
Its schema is the driver's append-only `MIGRATIONS` list in
`driver/sqlite/schema/mod.rs`, naming each table's steps from
`schema/<table>.rs` (version 1: one
`STRICT` `note` table, timestamps as RFC 3339 UTC text, tags as a JSON
array in a text column checked by `json_valid`), keyed on
`PRAGMA user_version`. Every read and write first checks that the data
directory belongs to this user, is open to no group or other permission bit
and is not a link; the kit checks the database and its sidecars the same
way. A refusal names the `chmod` that fixes it.

A read opens one read-only connection that holds one read transaction, so
every query in one command sees one committed version; it takes no lock,
changes no note, and works on a read-only data directory. A missing data
directory or database reads as an empty store and creates nothing. Each row
is mapped to the contract types at the construction boundary with no
default: a row whose title, tag, priority or timestamp fails validation is
isolated as a `BadRecord` (`store_bad_record`) naming its id and reason;
reads return the valid notes plus per-record warnings, `show <bad id>` fails
with that record's error, and `add` refuses while any bad record exists, so
a bad record is repaired before anything is added beside it.

The `add` flow reads the store once before touching anything, so a refused
store leaves nothing behind, and closes that read. It then calls
`Store::write`, which creates the data directory (`0700`), opens the
database (creating it `0600` and running any schema step it lacks), begins
one `IMMEDIATE` transaction (waiting up to 10 seconds for another writer,
then `store_locked`), refuses a store with bad records, and runs the flow:
it assigns the next id (`rule::next_id`: checked max id + 1, refusing with
`note_id_exhausted` at `NoteId::MAX`, SQLite's largest integer) and puts the
note. The transaction commits when the flow succeeds and rolls back when it
fails. A database at a newer schema version is refused for reads and writes
(`store_too_new`) and left as it is.

## 5. The audit log

Every command that changes durable state is audited; today that is `note
add`. The app's `Command::audit` declares it per command with an exhaustive match, so a
new command does not compile until it decides. Read-only commands are not
audited, because they must not write (STD-01 R31).

`AuditGuard` (in the core's `runtime/audit.rs`, modelled on Orbit's) is
created by the runtime's `exec` before the flow runs, in the failure state, so an early return or a panic is
recorded as a failure, never as success. `mark_result` sets the outcome, the
note id on success and the stable error code on failure; `Drop` runs after
`output::finish` and appends one line to `<root>/audit.jsonl` through
the audit repository (`Store::audit().append`), which checks the data directory like any write, holds
`.audit.lock` (apart from the database, so a lock timeout on a note write
still records its failure line within one deadline) and creates the file
`0600`.
Each line is `{"format": 2, "at", "actor", "command", "status", "target",
"error_code", "duration_ms"}`: identifiers only, never argument values or
error text (STD-05 R13). `actor` is the OS user name read once in `Env`
from `USER`, else `USERNAME`, serialized as `null` when neither is set; older
format-1 lines without `actor` upgrade in memory via `db/audit.rs::UPGRADES`
with `"actor": null`. The log is a side channel, so it fails open (STD-02
R31): a line that cannot be written is a `warn` on stderr (after the command's
own output or JSON error object) and never changes the command's result or
exit code.

## 6. Gates

| Gate | Make target | Holds |
|------|-------------|-------|
| `cargo fmt --check` on stable, with `rustfmt.toml` at `max_width = 100` and only stable options | `ci-fast` | formatting (STD-12 R11); an option stable rustfmt calls unstable fails the gate |
| `check-dependency-direction.sh` | `ci-fast` | crate and module layering (STD-02 R1–R6), file access only in the store, `rusqlite` only in the store and only through `cw_sqlite`, schema statements only in `driver/sqlite/schema/` and step lists only in its `mod.rs`, no repository path named in the core, environment reads only in the core's `composition.rs`, no clock-defaulted timestamps (R16), retired paths stay gone (R17) |
| `check-test-modules.sh` | `ci-fast` | every sibling unit-test file is declared (STD-02 R19) |
| `check-terminal-guard.sh` | `ci-fast` | one owner for streams, TTY and color env (STD-01 R12, R17) |
| `check-singular-names.sh` | `ci-fast` | module, source directory and source file names are singular, apart from its allow-list (STD-12 R1) |
| `check-source-style.sh` | `ci-fast` | what stable rustfmt cannot check: blank lines between the module doc, the use block, top-level items and impl methods (STD-12 R12), lines within `max_width` apart from one string literal or URL (R13), one module per `use` (R16), a reason before each `rustfmt::skip` (R19) |
| `check-private-task-ids.sh` | `ci-fast` | public tree excludes private task IDs outside the vendored canon |
| `check-readme.py` and `generate-feature-board.py --check` | `ci-fast` | README size and local links (STD-09 R2, R12), the design feature board (STD-06 R9) |
| `docs/standards/check.sh` | `ci-fast` (`standards-check`) | vendored standards (warns until vendored; fails in CI) |
| `clippy --all-targets -D warnings` with the STD-02 lint table and its adoptable additions, plus STD-12's lints and `clippy.toml` | `ci-lint` | no unwrap/expect/print/dbg, no lock across await, no wildcard enum arms, no narrowing `as`; no one-letter binding (`min_ident_chars`, no allowed idents; STD-12 R14), no glob import (`wildcard_imports`, `enum_glob_use`; R15), every public item documented (`missing_docs`; R18) |
| `cargo deny check` | `ci-lint` | advisories, licenses, sources |
| nextest `--no-tests=fail` (timeouts in `.config/nextest.toml`, no retries), then `cargo test --doc` | `test` | behaviour, and runnable doc examples |
| help and output goldens from the built binary | `goldens` | the public surface (STD-01 R24) |

Cargo builds and tests pass `--locked`. When Cargo's advisory cache is
read-only, the supply-chain check uses `target/cargo-deny/advisory-dbs`.
Missing gate tools warn locally and fail under `CI=true`. CI runs
`make ci` on Linux and macOS with a 30-minute job timeout and pins actions,
the toolchain and tools. Dependabot proposes crate and action updates.

## 7. Concerns & Honest Limitations

- Terminal width comes from exported `COLUMNS`. It controls table truncation
  and wrapping of root command summaries on a terminal. Piped help keeps
  one line per command. Sampling an unexported terminal width needs a
  dependency (`terminal_size`) the template does not take.
- Usage errors keep clap's multi-line rendering in human mode
  ([4_decisions.md](./4_decisions.md#usage-errors-keep-claps-rendering)).
- Under plain `cargo test` (nextest absent) there is no per-test timeout;
  only the CI job timeout bounds a hang.
- A note-write lock timeout (`store_locked`) names no holder: SQLite's locks
  record no owner. The audit lock's holder record is best effort: the kit
  writes it just after the lock is taken and clears it on release, and a
  failure to write it is logged as a warning.
- `note list` and `note add` read every row: fine for thousands of notes. A
  domain that outgrows that adds a query to the notes repository.
- The ownership and mode checks are unix-only; on Windows they are no-ops.
- The audit log grows without bound and has no reader command; rotating or
  querying it is left to the domain. A crash mid-append can leave a torn last
  line, which a reader must skip.
- An audit line records that a command ran, not that its output reached the
  reader: a `note add` whose stdout write then fails is still `success`,
  because the note was written.
- A store holding any bad record remains readable (valid notes plus stderr
  warnings), but writes refuse until the bad record is fixed by hand with
  SQL ([4_decisions.md](./4_decisions.md#bad-note-records-are-isolated-on-read-and-block-writes)).
- In a writable data directory a read may rebuild `notes.db-shm` (SQLite's
  index) and creates a missing `notes.db-wal` (empty) or `notes.db-shm`,
  owner-only, as SQLite needs them to read a WAL database. In a read-only
  directory a read writes nothing.
