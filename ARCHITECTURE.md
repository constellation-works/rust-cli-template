# Architecture

The layer model that `scripts/check-dependency-direction.sh` enforces
(STD-02 R1, R6). A lower layer never imports a higher one. Adding a crate or
an edge changes this file and that script in the same commit.

## Crates

```
tmpl-cli  ->  tmpl-cli-core  ->  tmpl-cli-store
                    |              |   |    |
                    +--> cw-base <-+   |    +--> cw-fs -> cw-base
                                       +--> cw-sqlite -> cw-fs, cw-base
```

`cw-base`, `cw-fs` and `cw-sqlite` are the cw-kit, with `cw-testkit` (test isolation, a
dev-dependency only), all from
[constellation-works/satellites](https://github.com/constellation-works/satellites),
a git dependency pinned in the root `Cargo.toml` to commit
`9c9cc998720f0a75041be3451c14fbaf80f29dc8` (no release is published yet; moving the
pin is a change of its own). Every kit crate is pinned to that one rev.

| Crate | Kind | Owns | Internal and kit dependencies | Banned |
|-------|------|------|-------------------------------|--------|
| `tmpl-cli` | binary `tmpl-cli` | Argument declarations, the mapping from arguments to an app's `Command`, rendering, exit codes. The only stdout, stderr and terminal access. | `tmpl-cli-core`; `cw-testkit` and `tmpl-cli-store` (its `test-support`) as dev-dependencies only | `tmpl-cli-store` as a normal dependency, every kit crate, `rusqlite` |
| `tmpl-cli-core` | library | The apps, the runtime they share, and composition. | `tmpl-cli-store`, `cw-base`; `cw-testkit` and the store's `test-support` as dev-dependencies only | `cw-fs`, `cw-sqlite`, `rusqlite`, `clap`, `tracing-subscriber`, any terminal crate, `tempfile`, `rustix` |
| `tmpl-cli-store` | library | The data directory: the notes database's driver, one repository per record, the record shapes (`contract`), the audit line, and the store's stable code for each of the kit's errors. It names no file primitive and no SQLite crate of its own: it reaches the directory and the audit log through `cw-fs` and the notes database through `cw-sqlite`, and names `rusqlite` only through its re-export. | `cw-base`, `cw-fs`, `cw-sqlite`; `cw-testkit` as a dev-dependency only | `clap`, `tracing-subscriber`, any terminal crate, `tempfile`, `rustix`, `rusqlite` as a dependency of its own |

`scripts/check-dependency-direction.sh` reads these edges from Cargo's own
package graph (`cargo metadata --no-deps --locked`), so a quoted or renamed
dependency is seen by its package. It holds each crate's kit list: a normal
or build dependency on a `cw-*` crate outside it fails the check, and so
does a dev-dependency outside the kit list and the dev-only kit list. The
dev-only kit edges are `cw-testkit` from every crate (the private temp
root, the isolated child and the goldens of their tests; no non-test source
names `cw_testkit`). The binary's and the core's tests also take
`tmpl-cli-store` with its `test-support` feature, which runs raw SQL on the
notes database, so a test can set up a store the public API never writes
while no crate outside the store names `rusqlite`. The binary names only
`tmpl-cli-core`. The core re-exports the store's record shapes as
`tmpl_cli_core::contract` and the kit's error type as
`tmpl_cli_core::Error` and `tmpl_cli_core::Kind`, so a surface needs no
second dependency. Why the crates are laid out by app, and why the
foundation is the kit:
[docs/design/tmpl-cli/4_decisions.md](docs/design/tmpl-cli/4_decisions.md#the-template-is-laid-out-by-app-in-three-crates),
[the kit decision](docs/design/tmpl-cli/4_decisions.md#the-foundation-is-the-cw-kit).

## `tmpl-cli` (binary)

```
main.rs              capture the environment into the core's Env, run
lib.rs               parse, resolve the output sink, dispatch, render
cli.rs               the root command and the global flags
command/<noun>.rs    arguments -> app::<app>::Command; Outcome -> payload
output/              payloads, rendering, the sink, logging
error.rs             the exit code and the remedy for an error; the output failure
help.rs              grouped root help
```

A command handler does three things: map the parsed arguments to the app's
`Command`, call `tmpl_cli_core::app::<app>::exec`, and turn the `Outcome`
into an output payload. It opens no store, resolves no path, reads no
environment variable and decides nothing about a record. `main` captures the
environment into `Env` once; `output/` alone reads the presentation variables
(`TMPL_CLI_FORMAT`, `TMPL_CLI_LOG`, `NO_COLOR`, `CLICOLOR_FORCE`, `TERM`,
`COLUMNS`).

## `tmpl-cli-core`

```
lib.rs               pub mod app; Env; contract (re-exported from the store); Error, Kind (from cw-base)
app/
  note/
    mod.rs           Command, Outcome, Error, exec (forwards to runtime::exec), run: dispatch only
    add.rs list.rs show.rs   one flow per command
    rule.rs          the note rules: the next id, a stored note from a new one, list selection
runtime/
  mod.rs             Runtime (started from composition), Ctx (what a flow receives)
  exec.rs            the one chokepoint: the App trait and exec, which every app's exec forwards to
  audit.rs           the audit guard around a mutating command
composition.rs       Env, the data directory, the Store, the actor, the clock; its Error (no data directory)
test_support.rs      behind the `test-support` feature: Ctx and each app's run
```

- An app's public surface is `Command`, `Outcome`, its typed `Error` and
  `exec(env, command, finish)`. `exec` is one line: it forwards to
  `runtime::exec`, the one chokepoint, which starts the runtime, runs the
  command inside the audit guard when it mutates, and passes the result to
  `finish` before the guard writes its line. The app supplies what differs
  by implementing the runtime's `App` trait: whether a command is audited,
  its dispatch, and its outcome's record id for the audit line. No app starts the runtime or the guard itself
  (`scripts/check-dependency-direction.sh`). `run(ctx, command)` is the
  dispatch without the runtime start or the guard; tests call it.
- A flow loads from one `store.read()`, decides with the app's pure `rule`,
  and commits in one `store.write(|tx| …)`, re-checking inside the closure
  anything that must be atomic with the write.
- An app may call another only through that app's `run` or what its `mod.rs`
  exports. There is no `app/common`, `app/shared` or `app/util`: code two
  apps need goes to the lower app, or to `runtime` when it is about the
  context, the chokepoint or the audit guard.
- `runtime` is crate-private and imports no app.
- Errors: each unit keeps a typed enum for its own matching (the app's
  `Error`, composition's, the store's) and translates it into
  `cw_base::Error` in one `From` impl beside the enum, naming its `Kind` and
  its stable code. `exec` returns that type, whatever the app; `run` returns the app's typed one. An
  app's enum holds its own refusals and one `Store(#[from] cw_base::Error)`
  variant, so a flow uses `?` on a store call with no hand-written
  conversion. What crosses a crate boundary is `cw_base::Error`: every public
  store operation returns it, translated at the store's edge, and a caller
  that needs the store's variant gets it back with `downcast_ref`.
- `composition.rs` is the only place in the core that reads an environment
  variable or discovers a path. Nothing in the core touches a file: the data
  directory is reached through the store.

## `tmpl-cli-store`

```
lib.rs               re-exports: Store, Snapshot, Tx, Error, contract, repository, STORE_FILE
store.rs             Store::open, read() -> Snapshot, write(|tx| …), audit()
driver/
  mod.rs             one module per storage mechanism
  sqlite/mod.rs      the notes database: STORE_FILE, open() -> cw_sqlite::Db; no query, no schema statement
  sqlite/schema/
    mod.rs           MIGRATIONS: the one append-only ordered list of every table's steps, each named
    note.rs          the note table's steps as named items (CREATE_NOTE)
repository/
  mod.rs             Snapshot (one reader), Tx (one write transaction)
  note.rs            notes: the SQL and row mapping; all, get, bad records; put
  audit/
    mod.rs           audit: append; the audit log's lock file name (AUDIT_LOCK_FILE)
    line.rs          the persisted audit line and its upgrades; bytes only, no I/O
contract/
  note.rs            Note, NoteId (1 to NoteId::MAX), Title, Body, Tag, Priority, NoteError
  audit.rs           AuditEvent, AuditStatus
error.rs             Error (a code for every cw_fs::Error and cw_sqlite::Error variant), its translator into cw_base::Error; BadRecord and its locator and reason
test_support.rs      behind the `test-support` feature: raw SQL on the notes database, for other crates' tests
```

- **A driver sets one storage mechanism up once and returns a handle; a
  repository is one record's use of it.** `driver/sqlite/` holds only this
  project's part of SQLite and returns the kit's `cw_sqlite::Db`; the kit is
  the mechanism. Its `mod.rs` holds the file name and `open()`, with no
  query and no schema statement. Its `schema/` holds one module per table,
  each exporting that table's steps as named items, and in `schema/mod.rs`
  the one append-only `MIGRATIONS` list naming every step in order. A step
  added later to an earlier table goes at the end of that list; per-table
  lists are never concatenated, so no shipped step is renumbered (STD-03
  R23). A step that changes two tables lives in the file of the table it
  creates or alters, and `schema/mod.rs` still sets its place in the order.
  A statement that changes the schema appears only under `schema/`.
  `repository/note.rs` holds the notes' SQL and the mapping between a row
  and the contract types, run on the reader `Store::read` opened or the
  transaction `Store::write` began. There is no `driver/fs/`: the audit
  repository reaches its file through `cw_fs` directly.
- **The record-access seam.** A caller reaches a record only through
  `snapshot.<record>()` in a read and `tx.<record>()` in a write; today
  that is `notes()`. The store's public surface names no `rusqlite` or
  `cw_sqlite` type, carries no borrow from a connection or a document, and
  names no path but `Store::path()` (the note app names the database in its
  outcome and refusals) and `Store::audit_path()` (the core's audit guard
  names the log in its warning). Repository calls return owned values.
- **A second driver.** There is no trait per repository and no driver chosen
  at run time. When a second storage mechanism is added, `driver/<other>/`
  sets it up, and `repository/note.rs` becomes `repository/note/mod.rs`
  (the record's public surface, unchanged for the core), `note/sqlite.rs`
  and `note/<other>.rs`.
- One repository per record, never per command. The store holds no rule of
  any app: no next-id rule, no list selection, no validation beyond what a
  contract type enforces on itself.
- `contract/` holds shapes only: type definitions, the constructor or
  parser that enforces the type's own invariant, serde and `Display`. No
  function that decides across records, no `check_*`, no clock, no file and
  no SQLite.
- A write refuses a store holding bad records, so a bad record is repaired
  before notes are added beside it; checks a flow wants before the data
  directory or the database exists run on its own read first.
- `Store` is one concrete type.

## The cw-kit (`cw-base`, `cw-fs`, `cw-sqlite`)

Not part of this workspace: crates from
[constellation-works/satellites](https://github.com/constellation-works/satellites)
at the pinned commit. Each crate's README there is its contract.

- `cw-base`: `Error` wraps a unit's typed error whole, with a `Kind` and the
  project's stable code. Its `Display` and source chain are the typed
  error's own; `downcast_ref` gives the typed error back. `Kind` is
  exhaustive on purpose (`Usage`, `NotFound`, `Refused`, `Busy`, `Corrupt`,
  `Incompatible`, `Config`, `Io`, `Internal`), so a new kind breaks every
  surface's match until it decides.
- `cw-fs`: atomic write, append, bounded read, listing, removal, private
  directories, owner-only checks and one advisory lock with a deadline;
  mechanism only, knowing no record and no file name. Its `Error` has a
  `Kind` and no code: the store wraps it as `Error::Fs` and names a code for
  every variant.
- `cw-sqlite`: one private SQLite database: WAL, owner-only files, a
  read-only reader per read, one `IMMEDIATE` write transaction per write, and
  an append-only list of migrations keyed on `PRAGMA user_version`;
  mechanism only, knowing no table. It bundles SQLite and re-exports
  `rusqlite`. Its `Error` has a `Kind` and no code: the store wraps it as
  `Error::Sqlite` and names a code for every variant.

## Module rules

Checked by grep over non-test sources, except that `rusqlite` and
`cw_sqlite` are refused outside `tmpl-cli-store` in tests too:

- `tmpl-cli-store`: nothing names `std::fs`, `std::net`, `std::process`,
  `tempfile` or `rustix`, and `rusqlite` only as `cw_sqlite::rusqlite`.
  A statement that changes the schema (`CREATE`, `ALTER` or `DROP` of a
  table, index, view or trigger, `PRAGMA user_version`, a `Step`) appears
  only under `driver/sqlite/schema/`, and only `schema/mod.rs` declares a
  list of steps. No public item names a `rusqlite` or
  `cw_sqlite` type. `repository/audit/line.rs` names neither `cw_fs` nor
  `cw_sqlite`. `contract/` imports no `driver`, `repository`, `Store`, file access
  or SQLite, and defines no `check_*` function and reads no clock. There is
  no `db/` and no `driver/fs/`. The crate reads no environment
  variable and discovers no path.
- `tmpl-cli-core`: no `std::fs`, `std::net`, `std::process`, `tempfile`,
  `rustix`, `cw_fs`, `cw_sqlite` or `rusqlite` anywhere, and no
  `tmpl_cli_store::repository` path: a record is reached through
  `snapshot.<record>()` or `tx.<record>()`. Only `composition.rs` names `std::env`, `home_dir` or
  `current_dir`. `runtime/` imports no `crate::app`. No `app/common`,
  `app/shared` or `app/util`. An app's `rule` names no `runtime`,
  `composition`, `Store`, `Snapshot` or `Tx`.
- `tmpl-cli`: nothing names `tmpl_cli_store`, `cw_base`, `cw_fs`, `cw_sqlite` or `rusqlite`. `command/` names no `std::env`,
  `std::fs`, `Store`, `Runtime` or `Ctx`. `output/` imports no `cli` or
  `command`.
- Only `tmpl-cli/src/output/` touches stdout, stderr, `IsTerminal` or the
  color/format variables (`scripts/check-terminal-guard.sh`).
- Every crate: module, source directory and source file names are singular
  (`scripts/check-singular-names.sh`, STD-12 R1), and the source style that
  stable rustfmt cannot check holds (`scripts/check-source-style.sh`, STD-12
  R12, R13, R16, R19).
