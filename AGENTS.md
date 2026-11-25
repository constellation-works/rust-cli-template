# tmpl-cli — agent guide

Loaded as both `AGENTS.md` and `CLAUDE.md` (a symlink). tmpl-cli keeps short notes in a local store; it is a Rust CLI built from the constellation Rust CLI template.

## Starting a project from this template

Keep the architecture from the first command, even when the entire first feature would fit in one file. Replace the example domain in place. Follow a command from the CLI handler through the core app, runtime and composition to the store's record repositories. Use the shared kit for infrastructure and carry the dependency checks and tests into the new project. Keep this guidance when you update the project name and description.

This structure comes from growing Scribe and Orbit. Moving file access, queries and application rules out of handlers after more commands depend on them costs repeated refactors across projects. Starting with the boundaries in place gives the second feature an example it can follow.

Before the first commit, trace one real command against [ARCHITECTURE.md](ARCHITECTURE.md) and run the template gates. The upstream template evolves. Use the integrated revision you generated from as the baseline and check its design decisions when a layout is in transition.

## Rules

- Work only on authorized scope. Neither implementation nor a PR request authorizes merging.
- Don't add a crate, or a dependency between crates, without updating [`ARCHITECTURE.md`](ARCHITECTURE.md) and `scripts/check-dependency-direction.sh` in the same change (STD-02 R6).
- Update affected docs in the same PR as the code. Stale docs are a review blocker. Edit `AGENTS.md` only when a rule, a command or a pointer in it changes; a new feature is described in its own docs.
- Don't touch `CHANGELOG.md` during tasks; it is compiled at release time.
- Use integration tests or goldens first (STD-04 R23). Every test must exercise behavior (STD-02 R21). No text-matching tests (`include_str!` plus `contains()` over source or copy).
- A fix comes with a regression test through the real entry point (the built binary or the public API), seen failing before the fix (STD-04 R1, R2).
- Tests never touch real host state: a private temp root is `cw_testkit::TempRoot`; the binary fixture and the kit-based script checks run their child through `TempRoot::command` (its environment cleared to `HOME`, `PATH`, `TMPL_CLI_ROOT` and `GIT_CEILING_DIRECTORIES`), while the copied-script checks in `tests/script.rs` run their scripts with their own commands in a plain temp directory; goldens compare through `cw_testkit::Goldens` ([test support](crates/tmpl-cli/tests/support/mod.rs); STD-03 R20). `cw-testkit` is a dev-dependency only.

## Branching

- `main` — releases only. `agent-main` — dev integration; every task PR targets it.

## Gates

- `make ci-fast` (no compile; `make help` lists its checks), `make ci-lint` and `make goldens` must pass before a task moves to review.
- After an intended change to help or output, run `make goldens UPDATE=1` and review the diff as a contract change.
- `make ci` (everything, including tests and doctests) runs in CI on Linux and macOS.

## Code

- Layers: [`ARCHITECTURE.md`](ARCHITECTURE.md). Design and decisions: [`docs/design/tmpl-cli/`](docs/design/tmpl-cli/); new features follow the [design conventions](docs/design/CONVENTIONS.md).
- Three crates: `tmpl-cli -> tmpl-cli-core -> tmpl-cli-store`, over the cw-kit from `constellation-works/satellites`, a git dependency pinned to one commit in the root `Cargo.toml`: `cw-base` (the one error type; the store and the core name it), `cw-fs` (file access; only the store names it) and `cw-sqlite` (the notes database; only the store names it, and `rusqlite` only through its re-export). A command handler maps arguments to the app's `Command`, calls `app::<app>::exec` and turns the `Outcome` into a payload; nothing else. The core holds the apps ([`crates/tmpl-cli-core/src/app/`](crates/tmpl-cli-core/src/app/)), the runtime and `composition.rs`, the only code that reads the environment. A flow loads from one store read, decides with the app's pure `rule`, and commits in one store write. No template crate names `std::fs`. In the store, `driver/sqlite/` sets the notes database up (its file and lock wait in `mod.rs`; in `schema/`, one module per table and the one ordered step list) and returns the `cw_sqlite::Db`; each repository (one per record) holds that record's SQL and row mapping; `contract/` holds record shapes; none holds an app's rule. The core reaches a record only through `snapshot.<record>()` and `tx.<record>()`.
- A noun's flags and handlers live in one module under [`crates/tmpl-cli/src/command/`](crates/tmpl-cli/src/command/); a large noun becomes a directory there with one file per verb.
- Every mutating command is audited: declare it in the app's `Command::audit` (the match is exhaustive, so a new command must decide). Read-only commands return `None`; they never write (STD-01 R31). Audit lines carry ids and error codes, never argument values or messages.
- Lints come from `[workspace.lints]`: no `unwrap`/`expect` in production code (STD-02 R13), no `print!` (use `tracing`; STD-02 R15), no lock guard across `.await` (STD-03 R1), no wildcard arm on an enum (STD-02 R27), no narrowing `as` (STD-02 R28), no one-letter binding, no glob import and no undocumented public item (STD-12 R14, R15, R18). CI runs clippy with `-D warnings`.
- Only `crates/tmpl-cli/src/output/` touches stdout, stderr, TTY state or color/format env vars (STD-02 R15). stdout is payload only (STD-01 R12).
- Errors: each unit keeps a typed `thiserror` enum and translates it into `cw_base::Error` in one `From` impl beside the enum, stating its `Kind` and stable code (STD-02 R10 to R12). `exec` returns that one type; nothing converts an error at a call site. Exit codes follow STD-01 R20.
- `--json` field names, the list envelope (`notes`, `total`, `truncated`) and the store's persisted shape are contracts (STD-01 R10, STD-02 R16): add fields, never rename or retype them. A schema change adds a named step to its table's module under [`crates/tmpl-cli-store/src/driver/sqlite/schema/`](crates/tmpl-cli-store/src/driver/sqlite/schema/) and appends it to `MIGRATIONS` in `schema/mod.rs`, never renumbering a shipped step; only `schema/` holds a statement that changes the schema; an audit-line change bumps its format and appends to `UPGRADES` in [`crates/tmpl-cli-store/src/db/audit.rs`](crates/tmpl-cli-store/src/db/audit.rs) (STD-03 R23).
- Default to `pub(crate)`; workspace dependencies via `.workspace = true` (STD-02 R8, R9).
- Unit tests live in a sibling tests directory mirroring source file names, declared in its `mod.rs`; the crate-root tests directory is integration only (STD-02 R19, R20).
- Never put internal task, friction or record ids in help, examples or messages; use placeholders such as `<id>` (STD-01 R23).
- Store writes go through `Store::write`, one `IMMEDIATE` transaction through `cw_sqlite::Db::write` (the audit log through `cw_fs::append` under its own lock) (STD-03 R5, R6). Never rewrite a durable file in place. Reads go through one read-only `cw_sqlite` reader and take no lock. Store state is owner-only and checked on every read and write.
- Report commands and outcomes at handoff — passed, failed, not run — never "tested" (STD-04 R16).

<!-- constellation-standards:begin -->
<!-- Managed by the constellation's operations/scripts/sync-standards.sh; edits inside this block are overwritten. -->
## Constellation standards

This repository adopts these constellation standards, vendored read-only in `docs/standards/` (each a directory: `STD-nn.md` holds the rules, `why.md` and `checks.md` the reasons and gates):

- `STD-01@2` — [docs/standards/STD-01-cli-surface/STD-01.md](docs/standards/STD-01-cli-surface/STD-01.md)
- `STD-02@4` — [docs/standards/STD-02-rust-architecture-and-errors/STD-02.md](docs/standards/STD-02-rust-architecture-and-errors/STD-02.md)
- `STD-03@2` — [docs/standards/STD-03-concurrency-and-process-safety/STD-03.md](docs/standards/STD-03-concurrency-and-process-safety/STD-03.md)
- `STD-04@2` — [docs/standards/STD-04-testing-and-verification/STD-04.md](docs/standards/STD-04-testing-and-verification/STD-04.md)
- `STD-05@1` — [docs/standards/STD-05-security-boundaries/STD-05.md](docs/standards/STD-05-security-boundaries/STD-05.md)
- `STD-06@1` — [docs/standards/STD-06-design-docs-and-feature-tracking/STD-06.md](docs/standards/STD-06-design-docs-and-feature-tracking/STD-06.md)
- `STD-08@2` — [docs/standards/STD-08-economy/STD-08.md](docs/standards/STD-08-economy/STD-08.md)
- `STD-09@1` — [docs/standards/STD-09-readme/STD-09.md](docs/standards/STD-09-readme/STD-09.md)

Follow them; they are normative. To deviate from a rule, record a decision in `docs/design/<feature>/4_decisions.md` citing `STD-nn@<version> §Rn`; never edit `docs/standards/` (`sh docs/standards/check.sh` enforces this).
Reviewers check every change against the adopted standards and report violations as `STD-nn §Rn` with file:line evidence.
<!-- constellation-standards:end -->
