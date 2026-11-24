# Architecture

The layer model that `scripts/check-dependency-direction.sh` enforces
(STD-02 R1, R6). A lower layer never imports a higher one. Adding a crate or
an edge changes this file and that script in the same commit.

## Crates

```
tmpl-cli  ->  tmpl-cli-core  ->  tmpl-cli-store
                    |                  |
                    +--> tmpl-cli-common <--+
```

| Crate | Kind | Owns | Internal dependencies | Banned |
|-------|------|------|-----------------------|--------|
| `tmpl-cli` | binary `tmpl-cli` | Argument declarations, the mapping from arguments to an app's `Command`, rendering, exit codes. The only stdout, stderr and terminal access. | `tmpl-cli-core` | `tmpl-cli-store` |
| `tmpl-cli-core` | library | The apps, the runtime they share, and composition. | `tmpl-cli-common`, `tmpl-cli-store` | `clap`, `tracing-subscriber`, any terminal crate, `tempfile`, `rustix` |
| `tmpl-cli-store` | library | The data directory: the persisted document, one repository per record, and the record shapes (`contracts`). It names no file primitive; it reaches the directory through the foundation's `fs`. | `tmpl-cli-common` | `clap`, `tracing-subscriber`, any terminal crate, `tempfile`, `rustix` |
| `tmpl-cli-common` | library (foundation) | The one error type every unit translates into, and file access for a private data directory. The only crate that names `std::fs`. | — | every internal crate; `clap`, `tracing-subscriber`, `serde_json`, any terminal crate |

The binary names only `tmpl-cli-core`. The core re-exports the store's
record shapes as `tmpl_cli_core::contracts` and the foundation's error type
as `tmpl_cli_core::Error` and `tmpl_cli_core::Kind`, so a surface needs no
second dependency. Why the
crates are laid out by app, and why a foundation crate:
[docs/design/tmpl-cli/4_decisions.md](docs/design/tmpl-cli/4_decisions.md#the-template-is-laid-out-by-app-in-three-crates),
[the foundation decision](docs/design/tmpl-cli/4_decisions.md#one-error-type-and-file-access-live-in-a-foundation-crate).

## `tmpl-cli` (binary)

```
main.rs              capture the environment into the core's Env, run
lib.rs               parse, resolve the output sink, dispatch, render
cli.rs               the root command and the global flags
commands/<noun>.rs   arguments -> apps::<app>::Command; Outcome -> payload
output/              payloads, rendering, the sink, logging
error.rs             the exit code and the remedy for an error; the output failure
help.rs              grouped root help
```

A command handler does three things: map the parsed arguments to the app's
`Command`, call `tmpl_cli_core::apps::<app>::exec`, and turn the `Outcome`
into an output payload. It opens no store, resolves no path, reads no
environment variable and decides nothing about a record. `main` captures the
environment into `Env` once; `output/` alone reads the presentation variables
(`TMPL_CLI_FORMAT`, `TMPL_CLI_LOG`, `NO_COLOR`, `CLICOLOR_FORCE`, `TERM`,
`COLUMNS`).

## `tmpl-cli-core`

```
lib.rs               pub mod apps; Env; contracts (re-exported from the store); Error, Kind (from the foundation)
apps/
  note/
    mod.rs           Command, Outcome, Error, exec (forwards to runtime::exec), run: dispatch only
    add.rs list.rs show.rs   one flow per command
    rules.rs         the note rules: the next id, a stored note from a new one, list selection
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
- A flow loads from one `store.read()`, decides with the app's pure `rules`,
  and commits in one `store.write(|tx| …)`, re-checking inside the closure
  anything that must be atomic with the write.
- An app may call another only through that app's `run` or what its `mod.rs`
  exports. There is no `apps/common`, `apps/shared` or `apps/util`: code two
  apps need goes to the lower app, or to `runtime` when it is about the
  context, the chokepoint or the audit guard.
- `runtime` is crate-private and imports no app.
- Errors: each unit keeps a typed enum for its own matching (the app's
  `Error`, composition's, the store's) and translates it into the
  foundation's `Error` in one `From` impl beside the enum. `exec` returns the
  foundation's type, whatever the app; `run` returns the app's typed one. An
  app's enum holds its own refusals and one `Store(#[from] …)` variant, so a
  flow uses `?` on a store call with no hand-written conversion.
- `composition.rs` is the only place in the core that reads an environment
  variable or discovers a path. Nothing in the core touches a file: the data
  directory is reached through the store.

## `tmpl-cli-store`

```
lib.rs               re-exports: Store, Snapshot, Tx, Error, contracts, repository
store.rs             Store::open, read() -> Snapshot, write(label, |tx| …), audit()
db/
  format.rs          the persisted notes document, its format versions and upgrades
  audit.rs           the persisted audit line and its upgrades
repository/
  mod.rs             Snapshot (one read), Tx (one write under the lock)
  notes.rs           notes: all, get, bad records; put
  audit.rs           audit: append
contracts/
  note.rs            Note, NoteId, Title, Body, Tag, Priority, NoteError
  audit.rs           AuditEvent, AuditStatus
error.rs             Error, its translator into the foundation's Error; BadRecord and its locator and reason
```

- One repository per record, never per command. The store holds no rule of
  any app: no next-id rule, no list selection, no validation beyond what a
  contract type enforces on itself.
- `contracts/` holds shapes only: type definitions, the constructor or
  parser that enforces the type's own invariant, serde and `Display`. No
  function that decides across records, no `check_*`, no clock, no file.
- A write refuses a store holding bad records, since rewriting it would drop
  them; checks a flow wants before the data directory or lock file exists
  run on its own read first.
- `Store` is one concrete type; a trait is added when a second backend exists.

## `tmpl-cli-common` (foundation)

```
lib.rs               pub mod error; pub mod fs; re-exports Error, Kind
error/
  mod.rs             Error: a unit's typed error with its Kind and stable code
  kind.rs            Kind: how a failure is classified, by what the caller can do
fs/
  fsio.rs            atomic write, append, advisory lock, bounded reads, directory
                     listing, private removal, owner-only checks; knows no record
  error.rs           Error: what a file operation fails with, and its Kind
```

- It is grouped by subject in a folder each: no file beside `lib.rs`, and no
  folder named `util`, `helpers`, `types` or the like. It depends on no
  other crate of the workspace and reads no environment variable.
- `error/`: `Error` wraps a unit's typed error whole. Its `Display` and
  source chain are the typed error's own; `kind()` and `code()` are what a
  caller reads without knowing the unit, and `downcast_ref` gives the typed
  error back to a caller that does. Only a unit's translator builds one.
  `Kind` is exhaustive on purpose, so a new kind breaks every surface's
  match until it decides.
- `fs/` is the only code in the workspace that names `std::fs`. It is
  mechanism only: a caller hands it a path and bytes, and it knows no record
  and no file name. Its typed `Error` (I/O, locked, symlink, foreign owner,
  permissions) is wrapped by the calling unit, which gives each variant a
  stable code: the store's `Error::Fs`. `error/` never imports it.

## Module rules

Checked by grep over non-test sources:

- `tmpl-cli-common`: only `fs/` names `std::fs`, `std::net`, `std::process`,
  `tempfile` or `rustix`; every other folder also defines no `check_*`
  function, reads no clock and imports no `crate::fs`. Nothing names
  `std::env`. No file beside `lib.rs` at
  the top of `src/`, and no folder named for no responsibility.
- `tmpl-cli-store`: nothing names `std::fs`, `std::net`, `std::process`,
  `tempfile` or `rustix`. `db/format.rs` and `db/audit.rs` do not name the
  foundation's `fs`. `contracts/` imports no `db`, `repository`, `Store` or
  file access, and defines no `check_*` function and reads no clock. There
  is no `db/fsio.rs`. The crate reads no environment variable and discovers
  no path.
- `tmpl-cli-core`: no `std::fs`, `std::net`, `std::process`, `tempfile`,
  `rustix` or `tmpl_cli_common::fs` anywhere. Only `composition.rs` names `std::env`, `home_dir` or
  `current_dir`. `runtime/` imports no `crate::apps`. No `apps/common`,
  `apps/shared` or `apps/util`. An app's `rules` names no `runtime`,
  `composition`, `Store`, `Snapshot` or `Tx`.
- `tmpl-cli`: nothing names `tmpl_cli_store` or `tmpl_cli_common`. `commands/` names no `std::env`,
  `std::fs`, `Store`, `Runtime` or `Ctx`. `output/` imports no `cli` or
  `commands`.
- Only `tmpl-cli/src/output/` touches stdout, stderr, `IsTerminal` or the
  color/format variables (`scripts/check-terminal-guard.sh`).
