# Architecture

The layer model that `scripts/check-dependency-direction.sh` enforces
(STD-02 R1, R6). A lower layer never imports a higher one. Adding a crate or
an edge changes this file and that script in the same commit.

## Crates

```
tmpl-cli  ->  tmpl-cli-core  ->  tmpl-cli-store
```

| Crate | Kind | Owns | Internal dependencies | Banned |
|-------|------|------|-----------------------|--------|
| `tmpl-cli` | binary `tmpl-cli` | Argument declarations, the mapping from arguments to an app's `Command`, rendering, exit codes. The only stdout, stderr and terminal access. | `tmpl-cli-core` | `tmpl-cli-store` |
| `tmpl-cli-core` | library | The apps, the runtime they share, and composition. | `tmpl-cli-store` | `clap`, `tracing-subscriber`, any terminal crate, `tempfile`, `rustix` |
| `tmpl-cli-store` | library | The data directory: the persisted document, one repository per record, and the record shapes (`contracts`). The only crate that touches the data directory. | — | `clap`, `tracing-subscriber`, any terminal crate |

The binary names only `tmpl-cli-core`. The core re-exports the record shapes
it returns as `tmpl_cli_core::contracts`, so rendering needs no second
dependency. Why three crates laid out by app:
[docs/design/tmpl-cli/4_decisions.md](docs/design/tmpl-cli/4_decisions.md#the-template-is-laid-out-by-app-in-three-crates).

## `tmpl-cli` (binary)

```
main.rs              capture the environment into the core's Env, run
lib.rs               parse, resolve the output sink, dispatch, render
cli.rs               the root command and the global flags
commands/<noun>.rs   arguments -> apps::<app>::Command; Outcome -> payload
output/              payloads, rendering, the sink, logging
error.rs             app errors -> one-line message, code and exit code
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
lib.rs               pub mod apps; Env; Error; contracts (re-exported from the store)
apps/
  note/
    mod.rs           Command, Outcome, Error, exec (forwards to runtime::exec), run: dispatch only
    add.rs list.rs show.rs   one flow per command
    rules.rs         the note rules: the next id, a stored note from a new one, list selection
runtime/
  mod.rs             Runtime (started from composition), Ctx (what a flow receives)
  exec.rs            the one chokepoint: the App trait and exec, which every app's exec forwards to
  audit.rs           the audit guard around a mutating command
  errors.rs          Error: what every app can fail with (no data directory, a store failure)
composition.rs       Env, the data directory, the Store, the actor, the clock
test_support.rs      behind the `test-support` feature: Ctx and each app's run
```

- An app's public surface is `Command`, `Outcome`, `Error` and
  `exec(env, command, finish)`. `exec` is one line: it forwards to
  `runtime::exec`, the one chokepoint, which starts the runtime, runs the
  command inside the audit guard when it mutates, and passes the result to
  `finish` before the guard writes its line. The app supplies what differs
  by implementing the runtime's `App` trait: whether a command is audited,
  its dispatch, and its outcome's record id and its error's code for the
  audit line. No app starts the runtime or the guard itself
  (`scripts/check-dependency-direction.sh`). `run(ctx, command)` is the
  dispatch without the runtime start or the guard; tests call it.
- A flow loads from one `store.read()`, decides with the app's pure `rules`,
  and commits in one `store.write(|tx| …)`, re-checking inside the closure
  anything that must be atomic with the write.
- An app may call another only through that app's `run` or what its `mod.rs`
  exports. There is no `apps/common`, `apps/shared` or `apps/util`: code two
  apps need goes to the lower app, or to `runtime` when it is about the
  context, the chokepoint, the audit guard or errors.
- `runtime` is crate-private and imports no app.
- `composition.rs` is the only place in the core that reads an environment
  variable or discovers a path. Nothing in the core touches a file: the data
  directory is reached through the store.

## `tmpl-cli-store`

```
lib.rs               re-exports: Store, Snapshot, Tx, Error, contracts, repository
store.rs             Store::open, read() -> Snapshot, write(label, |tx| …), audit()
db/
  fsio.rs            atomic write, append, advisory lock, bounded reads, directory
                     listing, private removal, owner-only checks; knows no record
  format.rs          the persisted notes document, its format versions and upgrades
  audit.rs           the persisted audit line and its upgrades
repository/
  mod.rs             Snapshot (one read), Tx (one write under the lock)
  notes.rs           notes: all, get, bad records; put
  audit.rs           audit: append
contracts/
  note.rs            Note, NoteId, Title, Body, Tag, Priority, NoteError
  audit.rs           AuditEvent, AuditStatus
error.rs             Error, BadRecord and its locator and reason
```

- One repository per record, never per command. The store holds no rule of
  any app: no next-id rule, no list selection, no validation beyond what a
  contract type enforces on itself.
- `contracts/` holds shapes only: type definitions, the constructor or
  parser that enforces the type's own invariant, serde and `Display`. No
  function that decides across records, no `check_*`, no clock.
- A write refuses a store holding bad records, since rewriting it would drop
  them; checks a flow wants before the data directory or lock file exists
  run on its own read first.
- `Store` is one concrete type; a trait is added when a second backend exists.

## Module rules

Checked by grep over non-test sources:

- `tmpl-cli-store`: only `db/` names `std::fs`, `std::net`, `std::process`,
  `tempfile` or `rustix`. `db/fsio.rs` imports nothing from `contracts`,
  `repository`, the rest of `db` or the store. `db/format.rs` and
  `db/audit.rs` do no file I/O. `contracts/` imports no `db`, `repository` or
  `Store`, and defines no `check_*` function and reads no clock. The crate
  reads no environment variable and discovers no path.
- `tmpl-cli-core`: no `std::fs`, `std::net`, `std::process`, `tempfile` or
  `rustix` anywhere. Only `composition.rs` names `std::env`, `home_dir` or
  `current_dir`. `runtime/` imports no `crate::apps`. No `apps/common`,
  `apps/shared` or `apps/util`. An app's `rules` names no `runtime`,
  `composition`, `Store`, `Snapshot` or `Tx`.
- `tmpl-cli`: nothing names `tmpl_cli_store`. `commands/` names no `std::env`,
  `std::fs`, `Store`, `Runtime` or `Ctx`. `output/` imports no `cli` or
  `commands`.
- Only `tmpl-cli/src/output/` touches stdout, stderr, `IsTerminal` or the
  color/format variables (`scripts/check-terminal-guard.sh`).
