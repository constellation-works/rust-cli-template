# Architecture

The layer model that `scripts/check-dependency-direction.sh` enforces
(STD-02 R1, R6). A lower layer never imports a higher one. Adding a crate or
an edge changes this file and that script in the same commit.

## Crates

```
tmpl-cli  ->  tmpl-cli-core  ->  tmpl-cli-store
                    |              |        |
                    +--> cw-base <-+        +--> cw-fs -> cw-base
```

`cw-base` and `cw-fs` are the cw-kit, with `cw-testkit` (test isolation, a
dev-dependency only), all from
[constellation-works/satellites](https://github.com/constellation-works/satellites),
a git dependency pinned in the root `Cargo.toml` to commit
`320e07f8d41c218c57f57a9f50820a2b6c34fb98` (no release is published yet; moving the
pin is a change of its own). Every kit crate is pinned to that one rev.

| Crate | Kind | Owns | Internal and kit dependencies | Banned |
|-------|------|------|-------------------------------|--------|
| `tmpl-cli` | binary `tmpl-cli` | Argument declarations, the mapping from arguments to an app's `Command`, rendering, exit codes. The only stdout, stderr and terminal access. | `tmpl-cli-core`; `cw-testkit` as a dev-dependency only | `tmpl-cli-store`, every kit crate |
| `tmpl-cli-core` | library | The apps, the runtime they share, and composition. | `tmpl-cli-store`, `cw-base`; `cw-fs` and `cw-testkit` as dev-dependencies only | `cw-fs`, `clap`, `tracing-subscriber`, any terminal crate, `tempfile`, `rustix` |
| `tmpl-cli-store` | library | The data directory: the persisted document, one repository per record, the record shapes (`contracts`), and the store's stable code for each of the kit's file errors. It names no file primitive; it reaches the directory through `cw-fs`. | `cw-base`, `cw-fs`; `cw-testkit` as a dev-dependency only | `clap`, `tracing-subscriber`, any terminal crate, `tempfile`, `rustix` |

`scripts/check-dependency-direction.sh` reads these edges from Cargo's own
package graph (`cargo metadata --no-deps --locked`), so a quoted or renamed
dependency is seen by its package. It holds each crate's kit list: a normal
or build dependency on a `cw-*` crate outside it fails the check, and so
does a dev-dependency outside the kit list and the dev-only kit list. The
dev-only kit edges are `cw-testkit` from every crate (the private temp
root, the isolated child and the goldens of their tests; no non-test source
names `cw_testkit`), and `tmpl-cli-core` to `cw-fs` (its integration tests
match the kit's file error behind a store failure, while its sources never
name `cw_fs`). The binary names
only `tmpl-cli-core`. The core re-exports the store's record shapes as
`tmpl_cli_core::contracts` and the kit's error type as
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
lib.rs               pub mod apps; Env; contracts (re-exported from the store); Error, Kind (from cw-base)
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
error.rs             Error (a code for every cw_fs::Error variant), its translator into cw_base::Error; BadRecord and its locator and reason
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

## The cw-kit (`cw-base`, `cw-fs`)

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

## Module rules

Checked by grep over non-test sources:

- `tmpl-cli-store`: nothing names `std::fs`, `std::net`, `std::process`,
  `tempfile` or `rustix`. `db/format.rs` and `db/audit.rs` do not name
  `cw_fs`. `contracts/` imports no `db`, `repository`, `Store` or
  file access, and defines no `check_*` function and reads no clock. There
  is no `db/fsio.rs`. The crate reads no environment variable and discovers
  no path.
- `tmpl-cli-core`: no `std::fs`, `std::net`, `std::process`, `tempfile`,
  `rustix` or `cw_fs` anywhere. Only `composition.rs` names `std::env`, `home_dir` or
  `current_dir`. `runtime/` imports no `crate::apps`. No `apps/common`,
  `apps/shared` or `apps/util`. An app's `rules` names no `runtime`,
  `composition`, `Store`, `Snapshot` or `Tx`.
- `tmpl-cli`: nothing names `tmpl_cli_store`, `cw_base` or `cw_fs`. `commands/` names no `std::env`,
  `std::fs`, `Store`, `Runtime` or `Ctx`. `output/` imports no `cli` or
  `commands`.
- Only `tmpl-cli/src/output/` touches stdout, stderr, `IsTerminal` or the
  color/format variables (`scripts/check-terminal-guard.sh`).
