# tmpl-cli — agent guide

Loaded as both `AGENTS.md` and `CLAUDE.md` (a symlink). tmpl-cli keeps short notes in a local store; it is a Rust CLI built from the constellation Rust CLI template.

<!-- constellation-standards:begin -->
<!-- Managed by the constellation's operations/scripts/sync-standards.sh; edits inside this block are overwritten. -->
## Constellation standards

This repository adopts these constellation standards, vendored read-only in `docs/standards/` (each a directory: `STD-nn.md` holds the rules, `why.md` and `checks.md` the reasons and gates):

- `STD-01@2` — [docs/standards/STD-01-cli-surface/STD-01.md](docs/standards/STD-01-cli-surface/STD-01.md)
- `STD-02@3` — [docs/standards/STD-02-rust-architecture-and-errors/STD-02.md](docs/standards/STD-02-rust-architecture-and-errors/STD-02.md)
- `STD-03@2` — [docs/standards/STD-03-concurrency-and-process-safety/STD-03.md](docs/standards/STD-03-concurrency-and-process-safety/STD-03.md)
- `STD-04@1` — [docs/standards/STD-04-testing-and-verification/STD-04.md](docs/standards/STD-04-testing-and-verification/STD-04.md)
- `STD-05@1` — [docs/standards/STD-05-security-boundaries/STD-05.md](docs/standards/STD-05-security-boundaries/STD-05.md)

Follow them; they are normative. To deviate from a rule, record a decision in `docs/design/<feature>/4_decisions.md` citing `STD-nn@<version> §Rn`; never edit `docs/standards/` (`sh docs/standards/check.sh` enforces this).
Reviewers check every change against the adopted standards and report violations as `STD-nn §Rn` with file:line evidence.
<!-- constellation-standards:end -->

## Rules

- Work only on authorized scope. Neither implementation nor a PR request authorizes merging.
- Don't add a crate, or a dependency between crates, without updating [`ARCHITECTURE.md`](ARCHITECTURE.md) and `scripts/check-dependency-direction.sh` in the same change.
- Update affected docs in the same PR as the code. Stale docs are a review blocker.
- Don't touch `CHANGELOG.md` during tasks; it is compiled at release time.
- Every test must exercise behavior. No text-matching tests (`include_str!` plus `contains()` over source or copy).
- A fix comes with a regression test through the real entry point (the built binary or the public API), seen failing before the fix.
- Tests never touch real host state: temp roots, and `HOME`/`TMPL_CLI_*` set on the child command ([`tests/support`](crates/tmpl-cli/tests/support/mod.rs)).

## Branching

- `main` — releases only. `agent-main` — dev integration; every task PR targets it.

## Gates

`make ci-fast` (fmt, structure, `standards-check`), `make ci-lint` and `make goldens` must pass before a task moves to review. After an intended change to help or output, run `make goldens UPDATE=1` and review the diff as a contract change. `make ci` (everything, including tests and doctests) runs in CI on Linux and macOS.

## Code

- Layers: [`ARCHITECTURE.md`](ARCHITECTURE.md). Design and decisions: [`docs/design/tmpl-cli/`](docs/design/tmpl-cli/); new features copy [`docs/design/_templates/`](docs/design/_templates/).
- `tmpl-cli-core` is the domain: no clap, no terminal, no environment reads. Business logic (`note.rs`, `query.rs`) does no I/O; everything that touches disk is in `store/`. The CLI crate parses (`cli.rs`), composes (`app.rs`), dispatches (`commands/`, one module per noun holding its flags and handlers; a large noun becomes `commands/<noun>/` with one file per verb), audits (`audit_middleware.rs`) and renders (`output/`).
- Every mutating command is audited: declare it in `Command::audit` (the match is exhaustive, so a new command must decide). Read-only commands return `None`; they never write. Audit lines carry ids and error codes, never argument values or messages.
- Lints come from `[workspace.lints]`: no `unwrap`/`expect` in production code, no `print!` (use `tracing`), no lock guard across `.await`, no wildcard arm on an enum, no narrowing `as`. CI runs clippy with `-D warnings`.
- Only `crates/tmpl-cli/src/output/` touches stdout, stderr, TTY state or color/format env vars. stdout is payload only.
- Errors: typed `thiserror` enums; the core's `Error` crosses into `CliError` through its one `#[from]`. Exit codes: 0 ok, 1 failure, 2 usage.
- `--json` field names, the list envelope (`notes`, `total`, `truncated`) and the store's persisted shape are contracts: add fields, never rename or retype them. A persisted-shape change bumps the store format and appends a step to `store/format.rs::UPGRADES`.
- Default to `pub(crate)`; workspace dependencies via `.workspace = true`.
- Unit tests live in a sibling `tests/` directory mirroring source file names, declared in its `mod.rs`; crate-root `tests/` is integration only.
- Never put internal task, friction or record ids in help, examples or messages; use placeholders such as `<id>`.
- Store writes go through `store::fsio::write_atomic` (the audit log through `append_private`) under the store lock. Never rewrite a durable file in place. Reads never write or lock. Store state is owner-only and checked on every load.
- Report commands and outcomes at handoff — passed, failed, not run — never "tested".
