---
type: architecture
summary: The crates, their allowed and forbidden edges, each crate's modules, and the rules the structure gates hold them to.
created_date: 2026-09-26
updated_date: 2026-11-25
---

# Architecture

The layer model that `scripts/check-dependency-direction.sh` enforces
(STD-02 R1, R6). A lower layer never imports a higher one. Adding a crate or
an edge changes this file and that script in the same commit. Every crate
and top-level module has its place here, and every path written in code
formatting exists (`scripts/check-architecture-map.sh`, STD-02 R43).

## Crates

```
tmpl-cli  ->  tmpl-cli-core  ->  tmpl-cli-store
   |            |   |              |   |    |
   |            |   +--> cw-base <-+   |    +--> cw-fs -> cw-base
   |            |                      +--> cw-sqlite -> cw-fs, cw-base
   |            +--> cw-app -> cw-fs, cw-base
   +--> cw-cli -> cw-app, cw-base
   +--> cw-mcp -> cw-app, cw-base
```

`cw-base`, `cw-fs`, `cw-sqlite`, `cw-app`, `cw-cli` and `cw-mcp` are the cw-kit, with `cw-testkit` (test isolation, a
dev-dependency only), all from
[constellation-works/satellites](https://github.com/constellation-works/satellites),
a git dependency pinned in the root `Cargo.toml` to commit
`493a18f618e6bb11e2b01e430a648c246245562a` (no release is published yet; moving the
pin is a change of its own). Every kit crate is pinned to that one rev.

| Crate | Kind | Owns | Internal and kit dependencies | Banned |
|-------|------|------|-------------------------------|--------|
| `tmpl-cli` | binary `tmpl-cli` | `main` (the one read of the arguments; it calls the core's `capture_env()` for the environment, STD-02 R7), argument declarations, the mapping from arguments to an app's `Command`, each command's human view, settling the audit record and printing its warning, the remedy for each error. CLI stdout, stderr and terminal access go through `cw-cli`: the terminal decision, sink, error rendering with the exit code, logging setup and grouped root help are the kit's. The second surface, an MCP server (`tmpl-cli mcp`) with one tool per command and protocol output, is `cw-mcp`'s. | `tmpl-cli-core`, `cw-cli`, `cw-mcp`; `cw-testkit`, `cw-mcp` (its `test-support`) and `tmpl-cli-store` (its `test-support`) as dev-dependencies only | `tmpl-cli-store` as a normal dependency, every other kit crate, `rusqlite`, `tracing-subscriber`, any terminal crate, `rustix` |
| `tmpl-cli-core` | library | The apps, each a `cw_app::App`, and composition (the runtime every command runs against). | `tmpl-cli-store`, `cw-base`, `cw-app`; `cw-testkit`, `cw-fs` (the capacity the kit's audit reader takes) and the store's `test-support` as dev-dependencies only | `cw-fs` as a normal dependency, `cw-sqlite`, `cw-cli`, `cw-mcp`, `rusqlite`, `clap`, `tracing-subscriber`, any terminal crate, `tempfile`, `rustix` |
| `tmpl-cli-store` | library | The data directory: the database's driver, one repository per record, the record shapes (`contract`), and the store's stable code for each of the kit's errors. No audit code or file: the audit log is `cw-app`'s. It names no file primitive and no SQLite crate of its own: it reaches the directory through `cw-fs` and the database through `cw-sqlite`, and names `rusqlite` only through its re-export. | `cw-base`, `cw-fs`, `cw-sqlite`; `cw-testkit` as a dev-dependency only | `cw-cli`, `cw-mcp`, `clap`, `tracing-subscriber`, any terminal crate, `tempfile`, `rustix`, `rusqlite` as a dependency of its own |

`scripts/check-dependency-direction.sh` reads these edges from Cargo's own
package graph (`cargo metadata --no-deps --locked`), so a quoted or renamed
dependency is seen by its package. It holds each crate's kit list: a normal
or build dependency on a `cw-*` crate outside it fails the check, and so
does a dev-dependency outside the kit list and the dev-only kit list. The
dev-only kit edges are `cw-testkit` from every crate (the private temp
root, the isolated child and the goldens of their tests; no non-test source
names `cw_testkit`). The binary's and the core's tests also take
`tmpl-cli-store` with its `test-support` feature, which runs raw SQL on the
database, so a test can set up a store the public API never writes
while no crate outside the store names `rusqlite`. The binary names
`tmpl-cli-core` and, for its two surfaces, `cw-cli` and `cw-mcp`, which no
other crate names. The core re-exports the store's record shapes as
`tmpl_cli_core::contract` and the kit's error type as
`tmpl_cli_core::Error` and `tmpl_cli_core::Kind`, and what a surface needs
of `cw-app` (`Env`, `EnvVar`, `Origin`, `Surface`, `Finished`,
`AuditWarning`, and the `Runtime` alias), so a surface needs no second
dependency.
<!-- example:begin -->

Why the crates are laid out by app, and why the foundation is the kit:
[docs/design/tmpl-cli/4_decisions.md](docs/design/tmpl-cli/4_decisions.md#the-template-is-laid-out-by-app-in-three-crates),
[the kit decision](docs/design/tmpl-cli/4_decisions.md#the-foundation-is-the-cw-kit).
<!-- example:end -->

## `tmpl-cli` (binary)

```
main.rs              capture_env() from the core and the arguments, once; run
lib.rs               probe the streams, parse, open the kit's sink, start logging and the runtime, dispatch, print an audit warning
cli.rs               the root command, the global flags, the root help's groups (rendered by cw_cli::help::grouped); Invocation: a command, or mcp
command/<noun>.rs    arguments -> app::<app>::Command; exec; Outcome -> payload; render, then settle
<!-- example:begin -->
command/input.rs     how a text argument is read from a tool's JSON: through the type's own FromStr
<!-- example:end -->
mcp/                 the MCP surface: one cw_mcp::Tool per command, served by `tmpl-cli mcp`
  mod.rs             tools(): each command's Args as the input, its summary as the description, its handler as the call; server, serve
output/              payloads and each command's human view, written through the kit's sink
  mod.rs             finish: emit or report through cw_cli::Sink; usage errors, help and version
  common/            the payloads two or more commands share
    payload.rs       the payloads every mode renders
  status.rs          the status table
<!-- example:begin -->
  note.rs            the note payloads, the list table, the detail text, the priority's colour role
<!-- example:end -->
error.rs             the remedy for an error; the surfaces' own failures (output, usage, the MCP server)
```

A command handler maps the parsed arguments to the app's `Command`, calls
`tmpl_cli_core::app::<app>::exec` with the runtime and the `Origin`, turns
the `Outcome` into an output payload, has it rendered, and then settles the
audit record, handing back any `AuditWarning`. It opens no store, resolves
no path, reads no environment variable and decides nothing about a record.
`main` reads the arguments and has the core capture the environment once
(`tmpl_cli_core::capture_env()`, over the kit's `Env::capture()`), the only
read of the process environment; no read of it lives in this crate
(STD-02 R7). The presentation variables
(`TMPL_CLI_FORMAT`, `TMPL_CLI_LOG`, `NO_COLOR`, `CLICOLOR_FORCE`, `TERM`,
`COLUMNS`) are interpreted by `cw-cli` alone: `lib.rs` probes the streams
once and the kit's `Terminal::decide` makes the one decision about colour,
width and mode from that probe and the captured `Env`. The template keeps no
terminal detection, colour decision, sink, table layout or logging setup of
its own. Errors reach stderr and the exit code only through the kit's
`Sink::report`, which returns the `Exit` the error's kind calls for; the
template supplies only the remedy (`error.rs`).

The MCP surface is a second mapping onto the same commands, not a second
set. A tool's input is its command's clap `Args` struct, which also derives
`Deserialize` and `JsonSchema`, so the parser, `--help` and the tool's
schema come from one declaration (STD-01 R25); its description is the
command's summary. The tool's function wraps the input in the CLI's
`Command` and calls the same handler as the command line, so both reach the
core through `app::<app>::exec` and the kit's `cw_app::exec`; a tool holds
no rule, store call or file access. Its result is the value `--json` prints
(`Payload::json`), and a failure is the command's own error and code, which
`cw-mcp` answers as a tool error. The server runs against the runtime `run`
started from the captured `Env`, reads no environment of its own, and never
writes to stdout outside the protocol. Each call's `Origin` is the `mcp`
surface and the client's name, so the audit log tells the surfaces apart.
To add a tool for a new command, give its `Args` the same derives and add
one line to `tools()` in `mcp/mod.rs`; a test fails while a command has no
tool or a tool's parameters differ from its command's arguments.

## `tmpl-cli-core`

```
lib.rs               pub mod app; start, Runtime, APP; contract (from the store); Error, Kind (cw-base); Env, Origin, … (cw-app)
app/
  status/
    mod.rs           Command, Outcome, Error, exec (forwards to cw_app::exec); StatusApp: cw_app::App (nothing audited)
    show.rs          one flow per command: the data directory and the store format, read-only
<!-- example:begin -->
  note/              the example domain
    mod.rs           Command, Outcome, Error, exec, as for status; NoteApp: cw_app::App (audit, run, target)
    add.rs list.rs show.rs   one flow per command
    rule.rs          the note rules: the next id, a stored note from a new one, list selection
<!-- example:end -->
environment.rs       capture_env(): the one read of the process environment (Env::capture()), called once by main
composition.rs       start: the data directory (the kit's, from Env), the Store, Runtime = cw_app::Runtime<Store>, Ctx (what a flow receives); its Error (no data directory)
```

- An app's public surface is `Command`, `Outcome`, its typed `Error` and
  `exec(runtime, origin, command) -> Finished<Outcome>`. `exec` is one line:
  it forwards to the kit's chokepoint, `cw_app::exec`, which starts the
  audit record of a mutating command (as a failure until the command reports
  otherwise), runs it, translates its error once and returns the result with
  the pending record. The surface renders the result, then settles the
  record. The app supplies what differs by implementing `cw_app::App` on a
  crate-private marker: its state (the `Store`), whether a command is
  audited, its dispatch (`run`), and its outcome's record id for the audit
  line. Nothing calls `run` but the chokepoint
  (`scripts/check-dependency-direction.sh`). A flow receives the kit's
  `Ctx` over the store, and reads the clock and the actor from it.
- A flow loads from one `store.read()`, decides with the app's pure `rule`,
  and commits in one `store.write(|tx| …)`, re-checking inside the closure
  anything that must be atomic with the write.
- An app may call another only through what its `mod.rs` exports. There is
  no common, shared or util app: code two apps need goes to the lower app.
- Errors: each unit keeps a typed enum for its own matching (the app's
  `Error`, composition's, the store's) and translates it into
  `cw_base::Error` in one `From` impl beside the enum, naming its `Kind` and
  its stable code. `exec` returns that type, whatever the app; `run` returns the app's typed one. An
  app's enum holds its own refusals and one `Store(#[from] cw_base::Error)`
  variant, so a flow uses `?` on a store call with no hand-written
  conversion. What crosses a crate boundary is `cw_base::Error`: every public
  store operation returns it, translated at the store's edge, and a caller
  that needs the store's variant gets it back with `downcast_ref`.
- `composition.rs` builds the runtime from the captured `Env`: the data
  directory is the kit's pure function of it (`--root` > `TMPL_CLI_ROOT` >
  `$HOME/.tmpl-cli`). Nothing in the core reads the process environment,
  discovers a path or touches a file: the records are reached through the
  store and the audit log through the kit.

## `tmpl-cli-store`

```
lib.rs               re-exports: Store, Snapshot, Tx, Error, contract, repository, STORE_FILE
store.rs             Store::open, read() -> Snapshot (strict: creates no file), read_before_write() -> Snapshot, write(|tx| …)
driver/
  mod.rs             one module per storage mechanism
  sqlite/mod.rs      the database: STORE_FILE (store.db), open() -> cw_sqlite::Db; no record query, no schema statement
  sqlite/damage.rs   the one probe: is a main file without its -wal and -shm a damaged database (immutable, read-only)
  sqlite/schema/
    mod.rs           MIGRATIONS: the one append-only ordered list of every table's steps, each named
<!-- example:begin -->
    note.rs          the note table's steps as named items (CREATE_NOTE)
<!-- example:end -->
repository/
  mod.rs             Snapshot (one reader; path() and format()), Tx (one write transaction)
<!-- example:begin -->
  note.rs            notes: the SQL and row mapping; all, get, bad records; put
<!-- example:end -->
contract/
<!-- example:begin -->
  note.rs            Note, NoteId (1 to NoteId::MAX), Title, Body, Tag, Priority, NoteError
<!-- example:end -->
error.rs             Error (including Sqlite(cw_sqlite::Error), with a code for every cw_fs::Error and cw_sqlite::Error variant), its translator into cw_base::Error; BadRecord and its locator and reason
test_support.rs      behind the `test-support` feature: raw SQL on the database, for other crates' tests
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
  A record's repository (`repository/<record>.rs`) holds its SQL and the
  mapping between a row and the contract types, run on the reader `Store::read` opened or the
  transaction `Store::write` began. There is no file-system driver, and no audit
  repository: the audit log is `cw-app`'s.
- **The record-access seam.** A caller reaches a record only through
  `snapshot.<record>()` in a read and `tx.<record>()` in a write.
  `Snapshot::path()` exposes the database path, and `Snapshot::format()` its
  format; `Store::root()` and `Store::path()` expose the store paths to apps.
  The public store error's `Sqlite` variant carries `cw_sqlite::Error`, but no
  `rusqlite` type or connection/document borrow escapes. Repository calls
  return owned values.
- **A second driver.** There is no trait per repository and no driver chosen
  at run time. When a second storage mechanism is added, `driver/<other>/`
  sets it up, and `repository/<record>.rs` becomes
  `repository/<record>/mod.rs` (the record's public surface, unchanged for
  the core), `<record>/sqlite.rs` and `<record>/<other>.rs`.
- One repository per record, never per command. The store holds no rule of
  any app: no next-id rule, no list selection, no validation beyond what a
  contract type enforces on itself.
- `contract/` holds shapes only: type definitions, the constructor or
  parser that enforces the type's own invariant, serde and `Display`. No
  function that decides across records, no `check_*`, no clock, no file and
  no SQLite.
- A write refuses a store holding bad records, so a bad record is repaired
  before records are added beside it; checks a flow wants before the data
  directory or the database exists run on its own read first.
- `Store` is one concrete type.

## The cw-kit (`cw-base`, `cw-fs`, `cw-sqlite`, `cw-app`)

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
  read-only reader per read (opt-in strict, which creates and changes no file),
  one `IMMEDIATE` write transaction per write, and
  an append-only list of migrations keyed on `PRAGMA user_version`;
  mechanism only, knowing no table. It bundles SQLite and re-exports
  `rusqlite`. Its `Error` has a `Kind` and no code: the store wraps it as
  `Error::Sqlite` and names a code for every variant.
- `cw-app`: the command path, mechanism only. `App` (what a program supplies:
  state, commands, which are audited, how to run one, what an outcome acted
  on), `exec` (the chokepoint) returning `Finished` (render, then `settle()`;
  a failed audit write is an `AuditWarning` value), `Runtime` and `Ctx` (the
  state, the clock, the origin), `Env::capture()` (the only read of the
  process environment; `Env::from_vars` for tests) and `env.app(name)` (the
  program's variables and data directory), and `AuditLog`
  (`<root>/audit.jsonl` under `<root>/.audit.lock`, identifiers only,
  versioned lines, fails open).

## Module rules

Three mechanisms hold these rules, each where it is exact:

- **Cargo's package graph** (`scripts/check-dependency-direction.sh`, from
  `cargo metadata`): the crate edges in the table above, including the
  external crates a crate may not take.
- **clippy** (`clippy.toml`, with `clippy::disallowed_methods` and
  `clippy::disallowed_types` denied in `[workspace.lints.clippy]`): file
  access, the process environment and spawning or ending a process. Clippy
  resolves each call to its path, so an alias (`use std::env;` then
  `env::var(..)`, a renamed import, a re-export) is refused too, in every
  target, tests included. An exemption is a module-level
  `#![expect(..., reason = "...")]`, never an allow.
- **grep** over non-test sources (the same script): module-to-module rules,
  `crate::` imports (grouped ones too), `std::net`, and names that are not
  paths clippy can resolve. `rusqlite` and `cw_sqlite` are refused outside
  `tmpl-cli-store` in tests too. The same script runs
  `scripts/check-module-cycles.sh`, which reads the `crate::`, `super::`
  and `self::` paths (including grouped `use` trees) into an import graph
  and fails on a cycle at any module level (STD-02 R6, R39); code both ends
  need moves to the lower module.

By clippy, in every crate:

- No `std::fs` function, no `std::path::Path` I/O method and no `std::fs`
  file, directory, metadata or permission type: file access is `cw_fs`'s,
  named only by the store.
- No `std::env` function (`var`, `var_os`, `vars`, `vars_os`, `set_var`,
  `remove_var`, `current_dir`, `set_current_dir`, `home_dir`, `temp_dir`,
  `current_exe`, `args`, `args_os`) and no `Env::capture`: the environment
  is read once, in the core's `environment.rs`, and the arguments once, in the
  binary's `main.rs`; those are the two exemptions in production code.
- No `Command::output`, `status` or `spawn`, no `CommandExt::exec`, no
  `std::process::Child`, `exit` or `abort`: the template spawns no process
  (a project that must spawn one uses the kit's `cw_supervise`).
- Tests: an integration test file that sets up fixtures or runs the built
  binary expects the file and process bans for its whole module. A unit-test
  module keeps its file set-up in a `fixture` submodule that carries the
  expectation, so the tests themselves still cannot read the environment.

By the package graph: the store and the core take no path-finding crate
(`dirs`, `dirs-next`, `directories`, `home`), on top of their banned lists
above.

By grep:

- `tmpl-cli-store`: nothing names `std::net`, `tempfile` or `rustix`, and
  `rusqlite` only as `cw_sqlite::rusqlite`.
  A statement that changes the schema (`CREATE`, `ALTER` or `DROP` of a
  table, index, view or trigger, `PRAGMA user_version`, a `Step`) appears
  only under `driver/sqlite/schema/`, and only `schema/mod.rs` declares a
  list of steps. No public item names a `rusqlite` or
  `cw_sqlite` type. Nothing names the audit log, its lock or `cw_app`.
  `contract/` imports no `driver`, `repository`, `Store`, file access
  or SQLite, and defines no `check_*` function and reads no clock. There is
  no db module and no file-system driver.
- `tmpl-cli-core`: no `std::net`, `tempfile`, `rustix`, `cw_fs`,
  `cw_sqlite` or `rusqlite` anywhere, and no `tmpl_cli_store::repository`
  path: a record is reached through `snapshot.<record>()` or
  `tx.<record>()`. Every app's `mod.rs` forwards to `cw_app::exec::<`. No
  common, shared or util app. An app's `rule` names no
  `composition`, `cw_app`, `Ctx`, `Store`, `Snapshot` or `Tx`.
- Every crate: no source calls an app's `run` (`…App::run`) or writes the
  audit log (`AuditLog::new`, `.append(`, `cw_app::Pending`).
- `tmpl-cli`: nothing names `tmpl_cli_store`, `cw_base`, `cw_fs`, `cw_sqlite`, `cw_app` or `rusqlite`. `command/` names no `Env`, `Store`,
  `Ctx`, `Runtime::` or `start(`: it receives
  the runtime, never builds it. `output/` imports no `cli` or
  `command`.
- Only `tmpl-cli/src/output/` touches stdout, stderr, `IsTerminal` or the
  color/format variables (`scripts/check-terminal-guard.sh`), and it does
  so only through `cw-cli`. The MCP server's stdio is `cw-mcp`'s, and
  nothing in `mcp/` writes to a stream.
- Every crate: module, source directory and source file names are singular
  (`scripts/check-singular-names.sh`, STD-12 R1), and the source style that
  stable rustfmt cannot check holds (`scripts/check-source-style.sh`, STD-12
  R12, R13, R16, R19, R21, R22, R26).
