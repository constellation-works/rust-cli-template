<div align="center">
  <h1>tmpl-cli</h1>
<!-- template:begin -->
  <p>The starting point for a new constellation Rust CLI</p>
<!-- copy:   <p>One line: what tmpl-cli does and for whom.</p> -->
<!-- template:end -->
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-0.1.0-blue" alt="Version"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="MIT licence"></a>
</div>

<!-- template:begin -->
## Starting point

This repository is the starting point for a new constellation Rust CLI: three crates laid out by app over the cw-kit (`cw-base`, `cw-fs` and `cw-sqlite`, pinned to one commit), an output contract, a private local SQLite store, and gates and CI that meet the constellation standards. A `note` example shows every layer.

Create a copy with GitHub's **Use this template**, clone it, then run `scripts/init.sh <name> [--kind cli|cli-bare|workspace] [--repository <url>]` and `make ci`:

- `cli` (the default) keeps the full template with its `note` example.
- `cli-bare` keeps the layers with one read-only `status` command and no example domain.
- `workspace` keeps an empty Cargo workspace with the gates, CI, lints, agent guide and design-doc templates.

The script renames the project, removes what the kind does not keep, writes `template.toml` (the template version and the kind) and deletes itself. [`template.toml`](template.toml) here holds the template version; a change to a shipped file raises it.
<!-- copy: ## Starting point -->
<!-- copy: -->
<!-- copy: Generated from the constellation Rust CLI template, version __VERSION__, kind `__KIND__`; [`template.toml`](template.toml) records both. -->
<!-- template:end -->

<!-- crates:begin -->
The CLI is pre-1.0.

## Quick start

Prerequisites: a Rust toolchain with Cargo and a checkout of this repository.

```sh
cargo install --path crates/tmpl-cli --locked
```

See where it keeps its data, and the store's format once one exists.

```sh
tmpl-cli status
```
<!-- example:begin -->

Add a note and see it in the list.

```sh
tmpl-cli note add "Call the plumber" --tag home --priority high
tmpl-cli note list
```
<!-- example:end -->
<!-- crates:end -->

## Learn more

<!-- crates:begin -->
- [User guide](docs/usage.md) for commands and troubleshooting.
- [Configuration](docs/usage.md#configuration) for flags, environment variables and defaults.
<!-- crates:end -->
- [Architecture](ARCHITECTURE.md) for the crate layout and dependency rules.
- [Feature board](docs/design/FEATURES.md) for design documents and their delivery stages.
- [Changelog](CHANGELOG.md) for release history.

## Contributing

See the [contributing guide](docs/development.md) for the development gates and test workflow.

This repository follows the constellation standards vendored in [`docs/standards/`](docs/standards/). Outside the constellation umbrella the standards are plain vendored files that `docs/standards/check.sh` only verifies.

## Licence

This project uses the [MIT License](LICENSE).
