# Architecture

The layer model that `scripts/check-dependency-direction.sh` enforces
(STD-02 R1, R6). A lower layer never imports a higher one. Adding a crate or
an edge changes this file and that script in the same commit.

## Layers

| Tier | Unit | Owns | May depend on |
|------|------|------|---------------|
| 1. Contract types | `tmpl-cli-core::note` | `Note`, `NoteId`, `Title`, `Tag`, `Priority`; validating constructors; serde shapes. No I/O. | nothing internal |
| 1. Mechanisms | `tmpl-cli-core::fsio` | Atomic durable write; advisory lock guard; owner-only creation and load checks. Knows nothing about notes. | nothing internal |
| 2. Domain | `tmpl-cli-core::store`, `::error` | The note store: list, show, add, format upgrades; the crate's typed `Error`. | tier 1 |
| 3. Composition | `tmpl-cli::app` | Resolves the data directory (`--root` > `TMPL_CLI_ROOT` > `~/.tmpl-cli`), constructs the `Store`, supplies the clock. | `tmpl-cli-core` |
| 4. Surface | `tmpl-cli::cli`, `::command`, `::output` | Argument declarations; one store call per verb; rendering from payloads; the only stdout/stderr/TTY access. | tiers 1–3 |

## Crates

| Crate | Kind | Internal dependencies | Banned |
|-------|------|-----------------------|--------|
| `tmpl-cli-core` | library | — | `clap`, `tracing-subscriber` and any terminal crate |
| `tmpl-cli` | binary `tmpl-cli` | `tmpl-cli-core` | — |

Why two crates rather than one: [docs/design/tmpl-cli/4_decisions.md](docs/design/tmpl-cli/4_decisions.md#a-domain-library-crate-and-a-cli-crate).

## Module rules inside crates

Checked by grep over non-test sources:

- `tmpl-cli-core/src/note.rs` imports no `std::fs`, `std::net`, `std::process`, `store` or `fsio`.
- `tmpl-cli-core` reads no environment variables and discovers no paths (`std::env::`, `home_dir`, `current_dir`).
- `tmpl-cli/src/app.rs` imports no `cli`, `command` or `output`.
- `tmpl-cli/src/output/` imports no `app`, `cli` or `command`.
- Only `tmpl-cli/src/output/` touches stdout, stderr, `IsTerminal` or the color/format variables (`scripts/check-terminal-guard.sh`).
