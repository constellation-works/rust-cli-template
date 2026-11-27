# tmpl-cli — agent guide

<!-- template:begin -->
Loaded as both `AGENTS.md` and `CLAUDE.md` (a symlink). This repository is the starting point for a new constellation Rust CLI; `scripts/init.sh` turns a copy into a project of one kind (`cli`, `cli-bare` or `workspace`), and `template.toml` holds the template version, raised by every change to a shipped file.
<!-- copy: Loaded as both `AGENTS.md` and `CLAUDE.md` (a symlink). tmpl-cli: <one line — what it does and for whom>. Generated from the constellation Rust CLI template, version __VERSION__, kind `__KIND__` (recorded in `template.toml`). -->
<!-- template:end -->

## Starting a project from this template

Keep the architecture from the first command, even when the entire first feature would fit in one file. Replace the example domain in place. Follow a command from `main` and the CLI handler through the kit's chokepoint and the core app to the store's record repositories. Use the shared kit for infrastructure and carry the dependency checks and tests into the new project. Keep this guidance when you update the project name and description.

This structure comes from growing Scribe and Orbit. Moving file access, queries and application rules out of handlers after more commands depend on them costs repeated refactors across projects. Starting with the boundaries in place gives the second feature an example it can follow.

Before the first commit, trace one real command against [ARCHITECTURE.md](ARCHITECTURE.md) and run the template gates. The upstream template evolves. Use the integrated revision you generated from as the baseline and check its design decisions when a layout is in transition.

## Rules

- Work only on authorized scope. Neither implementation nor a PR request authorizes merging.
- Don't add a crate, or a dependency between crates, without updating [`ARCHITECTURE.md`](ARCHITECTURE.md) and `scripts/check-dependency-direction.sh` in the same change (STD-02 R6). Every crate and every top-level module has its place in `ARCHITECTURE.md` and a first-line `//!` doc; `scripts/check-architecture-map.sh`, part of `make structure`, fails on a unit the map misses, a path it names that does not exist, or a unit without that doc (STD-02 R43).
- Update affected docs in the same PR as the code. Stale docs are a review blocker. Edit `AGENTS.md` only when a rule, a command or a pointer in it changes; a new feature is described in its own docs.
- Don't touch `CHANGELOG.md` during tasks; it is compiled at release time.
- Use integration tests or goldens first (STD-04 R23). Every test must exercise behavior (STD-02 R21). No text-matching tests (`include_str!` plus `contains()` over source or copy).
- A fix comes with a regression test through the real entry point (the built binary or the public API), seen failing before the fix (STD-04 R1, R2).
<!-- crates:begin -->
- Tests never touch real host state: a private temp root is `cw_testkit::TempRoot`; every child runs through `TempRoot::command` (its environment cleared to `HOME`, `PATH`, `TMPL_CLI_ROOT` and `GIT_CEILING_DIRECTORIES`), with only named variables added, including the script-specific integration tests under `crates/tmpl-cli/tests/`; goldens compare through `cw_testkit::Goldens` ([test support](crates/tmpl-cli/tests/support/mod.rs); STD-03 R20). `cw-testkit` is a dev-dependency only.
<!-- crates:end -->
- Outside the constellation umbrella the standards are plain vendored files that `docs/standards/check.sh` only verifies.

## Branching

- `main` — releases only. `agent-main` — dev integration; every task PR targets it.

## Gates

- `make ci-fast` (no compile; `make help` lists its checks), `make ci-lint` and `make goldens` must pass before a task moves to review.
<!-- crates:begin -->
- After an intended change to help or output, run `make goldens UPDATE=1` and review the diff as a contract change.
<!-- crates:end -->
- `make ci` (everything, including tests and doctests) runs in CI on Linux and macOS.

## Code

- Layers: [`ARCHITECTURE.md`](ARCHITECTURE.md). New features follow the [design conventions](docs/design/CONVENTIONS.md).
<!-- example:begin -->
- Design and decisions: [`docs/design/tmpl-cli/`](docs/design/tmpl-cli/).
<!-- example:end -->
- Lints come from `[workspace.lints]`: no `unwrap`/`expect` in production code (STD-02 R13), no `print!` (use `tracing`; STD-02 R15), no lock guard across `.await` (STD-03 R1), no wildcard arm on an enum (STD-02 R27), no narrowing `as` (STD-02 R28), no `#[allow]` where an `#[expect(..., reason)]` would do and no `deny(warnings)` in source (STD-02 R35), at most one bool parameter (R36), no by-value parameter that is only read (R38), no one-letter binding, no glob import and no undocumented public item (STD-12 R14, R15, R18). CI runs clippy with `-D warnings`.
- Boundaries have one mechanism each ([`ARCHITECTURE.md`](ARCHITECTURE.md), Module rules): crate edges come from Cargo's package graph and module rules from grep, both in `scripts/check-dependency-direction.sh`; file access, the process environment and spawning a process are refused by clippy through `clippy.toml` (`disallowed_methods`, `disallowed_types`, denied). An exemption is a module-level `#![expect(..., reason = "...")]`, never an `allow`.
- Default to `pub(crate)`; workspace dependencies via `.workspace = true` (STD-02 R8, R9).
- Unit tests live in a sibling tests directory mirroring source file names, declared in its `mod.rs`; the crate-root tests directory is integration only (STD-02 R19, R20).
- Never put internal task, friction or record ids in help, examples or messages; use placeholders such as `<id>` (STD-01 R23).
<!-- crates:begin -->
- Three crates: `tmpl-cli -> tmpl-cli-core -> tmpl-cli-store`, over the cw-kit from `constellation-works/satellites`, a git dependency pinned to one commit in the root `Cargo.toml`: `cw-base` (the one error type; the store and the core name it), `cw-fs` (file access; only the store names it), `cw-sqlite` (the database; only the store names it, and `rusqlite` only through its re-export) `cw-app` (the chokepoint, the captured environment and the audit log; only the core names it and re-exports what a surface needs) and `cw-cli` (the command line's mechanism: the terminal decision, the output sink, error rendering with the exit code, logging and grouped help; only the CLI crate names it) and `cw-mcp` (the MCP server; only the CLI crate names it). `main` reads the arguments and captures the environment once through the core's `capture_env()` (`Env::capture()` in `crates/tmpl-cli-core/src/environment.rs`; STD-02 R7 keeps every read of the environment out of the CLI crate); clippy refuses every other read of the process. A command handler maps arguments to the app's `Command`, calls `app::<app>::exec` (one line forwarding to `cw_app::exec`), turns the `Outcome` into a payload, has it rendered and then settles the audit record; nothing else. The core holds the apps ([`crates/tmpl-cli-core/src/app/`](crates/tmpl-cli-core/src/app/)), each implementing `cw_app::App`, and `composition.rs`, which builds the runtime from the captured `Env`. A flow loads from one store read, decides with the app's pure `rule`, and commits in one store write. No template crate calls `std::fs`. In the store, `driver/sqlite/` sets the database up (its file and lock wait in `mod.rs`; in `schema/`, one module per table and the one ordered step list) and returns the `cw_sqlite::Db`; each repository (one per record) holds that record's SQL and row mapping; `contract/` holds record shapes; none holds an app's rule. The core reaches a record only through `snapshot.<record>()` and `tx.<record>()`.
- A noun's flags and handlers live in one module under [`crates/tmpl-cli/src/command/`](crates/tmpl-cli/src/command/); a large noun becomes a directory there with one file per verb.
- Every command is also an MCP tool (`crates/tmpl-cli/src/mcp/`): its clap `Args` struct derives `Deserialize` and `JsonSchema` as the tool's input, and one line in `tools()` maps it to the CLI's `Command`, so the tool calls the same handler. A new command gets its tool in the same change (a test fails otherwise); a tool never holds a rule, a store call or file access, and nothing in `mcp/` writes to stdout.
- Every mutating command is audited: declare it in the app's `Command::audit` (the match is exhaustive, so a new command must decide). Read-only commands return `None`; they never write (STD-01 R31). Audit lines carry ids and error codes, never argument values or messages. The audit log and its line format are the kit's (`cw_app::AuditLog`); the template writes no audit file of its own.
- Only `crates/tmpl-cli/src/output/` touches stdout, stderr, TTY state or color/format env vars (STD-02 R15), and only through `cw-cli`: `lib.rs` probes the streams once, the kit's `Terminal::decide` decides, and every byte goes through `cw_cli::Sink`. `output/` holds the payloads and each command's view (a `cw_cli::Table`, or a detail text), never a terminal check, colour decision, table layout or logging setup of its own. stdout is payload only (STD-01 R12).
- An environment variable is declared once, as a `tmpl_cli_core::EnvVar`: the kit's in `cw_app::env` (listed in its `KIT_VARS`), or the CLI's own, declared in the module that reads it and passed to `env_table()` in `crates/tmpl-cli/src/lib.rs`. It is read through that declaration; no other source file spells its name as a string. `docs/usage.md`'s variables table is rendered from the declarations: after adding one, run `make goldens UPDATE=1`, or `make ci` fails (STD-01 R25).
- Errors: each unit keeps a typed `thiserror` enum and translates it into `cw_base::Error` in one `From` impl beside the enum, stating its `Kind` and stable code (STD-02 R10 to R12). `exec` returns that one type; nothing converts an error at a call site. Errors reach stderr and the exit code only through `cw_cli::Sink::report` (STD-01 R19, R20); the surface supplies the remedy (`error.rs`, `hint`) and maps no kind to a code.
- `--json` field names and the store's persisted shape are contracts (STD-01 R10, STD-02 R16): add fields, never rename or retype them. A schema change adds a named step to its table's module under [`crates/tmpl-cli-store/src/driver/sqlite/schema/`](crates/tmpl-cli-store/src/driver/sqlite/schema/) and appends it to `MIGRATIONS` in `schema/mod.rs`, never renumbering a shipped step; only `schema/` holds a statement that changes the schema (STD-03 R23).
- Store writes go through `Store::write`, one `IMMEDIATE` transaction through `cw_sqlite::Db::write` (the audit log is the kit's, appended under its own lock) (STD-03 R5, R6). Never rewrite a durable file in place. Reads go through one read-only `cw_sqlite` reader and take no lock. Store state is owner-only and checked on every read and write.
<!-- example:begin -->
- The list envelope (`notes`, `total`, `truncated`, `warnings`) is a `--json` contract too.
<!-- example:end -->
<!-- crates:end -->
- Report commands and outcomes at handoff — passed, failed, not run — never "tested" (STD-04 R16).

<!-- constellation-standards:begin -->
<!-- Managed by the constellation's operations/scripts/sync-standards.sh; edits inside this block are overwritten. -->
## Constellation standards

This repository adopts these constellation standards, vendored read-only in `docs/standards/` (each a directory: `STD-nn.md` holds the rules, `why.md` and `checks.md` the reasons and gates):

- `STD-01@4` — [docs/standards/STD-01-cli-surface/STD-01.md](docs/standards/STD-01-cli-surface/STD-01.md): CLI surface — command grammar, inputs and effects, output modes, errors, help, and golden-tested public surfaces
- `STD-02@11` — [docs/standards/STD-02-rust-architecture-and-errors/STD-02.md](docs/standards/STD-02-rust-architecture-and-errors/STD-02.md): Rust architecture and errors — layering, visibility, typed errors, failure semantics, idiom and naming, logging, and…
- `STD-03@2` — [docs/standards/STD-03-concurrency-and-process-safety/STD-03.md](docs/standards/STD-03-concurrency-and-process-safety/STD-03.md): Concurrency and process safety — locks, channels, durable writes, state evolution, subprocess lifecycles, destructive…
- `STD-04@2` — [docs/standards/STD-04-testing-and-verification/STD-04.md](docs/standards/STD-04-testing-and-verification/STD-04.md): Testing and verification — test design, hermetic tests, honest gates, docs as contracts, and evidence-based…
- `STD-05@1` — [docs/standards/STD-05-security-boundaries/STD-05.md](docs/standards/STD-05-security-boundaries/STD-05.md): Security boundaries — authority, filesystem containment, child environments, redaction, local network surfaces, supply…
- `STD-06@3` — [docs/standards/STD-06-design-docs-and-feature-tracking/STD-06.md](docs/standards/STD-06-design-docs-and-feature-tracking/STD-06.md): Design docs and feature tracking — one folder per feature, a delivery stage separate from doc approval, a generated…
- `STD-08@4` — [docs/standards/STD-08-economy/STD-08.md](docs/standards/STD-08-economy/STD-08.md): Economy — bounded agent context, delta-only records, one channel per ask, bookkeeping off the page, one-line errors…
- `STD-09@4` — [docs/standards/STD-09-project-docs/STD-09.md](docs/standards/STD-09-project-docs/STD-09.md): Project docs — a shared structure for the README and common repository documentation
- `STD-12@4` — [docs/standards/STD-12-source-style/STD-12.md](docs/standards/STD-12-source-style/STD-12.md): Source style — names, vertical spacing, line width and the formatter gate, as a language-neutral core with one profile…

Follow them; they are normative. To deviate from a rule, record a decision in `docs/design/<feature>/4_decisions.md` citing `STD-nn@<version> §Rn`; never edit `docs/standards/` (`sh docs/standards/check.sh` enforces this).
Reviewers check every change against the adopted standards and report violations as `STD-nn §Rn` with file:line evidence.
<!-- constellation-standards:end -->
