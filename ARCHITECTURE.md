# Architecture

The layer model that `scripts/check-dependency-direction.sh` enforces
(STD-02 R1, R6). A lower layer never imports a higher one. Adding a crate or
an edge changes this file and that script in the same commit.

## Layers

| Tier | Unit | Owns | May depend on |
|------|------|------|---------------|
| 1. Business logic | `tmpl-cli-core::note`, `::query`, `::error` | `Note`, `NoteId`, `Title`, `Tag`, `Priority`; validating constructors; serde shapes; the next-id rule; list selection (filter, count, limit); the crate's typed `Error`. No I/O. | nothing internal |
| 2. Persistence | `tmpl-cli-core::store` | The `Store` facade (list, show, add, record an audit event) over its private parts: `format` (the persisted document, format upgrades), `audit` (the audit record shape) and `fsio` (atomic write, append, advisory lock, owner-only checks; knows nothing about notes). The only core module that touches the filesystem. | tier 1 |
| 3. Composition | `tmpl-cli::app` | Resolves the data directory (`--root` > `TMPL_CLI_ROOT` > `~/.tmpl-cli`), constructs the `Store`, supplies the clock. | `tmpl-cli-core` |
| 4. Surface | `tmpl-cli::cli`, `::commands`, `::audit_middleware`, `::output` | Root argument declarations; one module per noun with its flags and handlers (one store call per verb); the audit guard around every mutating command; rendering from payloads; the only stdout/stderr/TTY access. | tiers 1–3 |

## Crates

| Crate | Kind | Internal dependencies | Banned |
|-------|------|-----------------------|--------|
| `tmpl-cli-core` | library | — | `clap`, `tracing-subscriber` and any terminal crate |
| `tmpl-cli` | binary `tmpl-cli` | `tmpl-cli-core` | — |

Why two crates rather than one: [docs/design/tmpl-cli/4_decisions.md](docs/design/tmpl-cli/4_decisions.md#a-domain-library-crate-and-a-cli-crate).
Why persistence is a module inside the core rather than a third crate:
[the store-module decision](docs/design/tmpl-cli/4_decisions.md#persistence-is-one-module-inside-the-core-crate).

## Module rules inside crates

Checked by grep over non-test sources:

- Every `tmpl-cli-core` module except `store/` (today `note.rs`, `query.rs`, `error.rs`, `lib.rs`, and any module added later) imports no `std::fs`, `std::net`, `std::process`, `tempfile` or `rustix`, and no `crate::store`.
- `tmpl-cli-core/src/store/fsio.rs` imports nothing from `note`, `query` or the rest of `store`; `store/format.rs` and `store/audit.rs` do no file I/O.
- `tmpl-cli-core` reads no environment variables and discovers no paths (`std::env::`, `home_dir`, `current_dir`).
- `tmpl-cli/src/app.rs` imports no `cli`, `commands`, `audit_middleware` or `output`.
- `tmpl-cli/src/output/` imports no `app`, `cli`, `commands` or `audit_middleware`.
- `tmpl-cli/src/audit_middleware.rs` imports no `cli` or `commands`: dispatch depends on it, not the reverse.
- Only `tmpl-cli/src/output/` touches stdout, stderr, `IsTerminal` or the color/format variables (`scripts/check-terminal-guard.sh`).
