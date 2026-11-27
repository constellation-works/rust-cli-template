<div align="center">
  <h1>tmpl-cli</h1>
<!-- template:begin -->
  <p>The starting point for a new constellation Rust CLI</p>
<!-- copy:   <p>A Rust command-line app starter.</p> -->
<!-- template:end -->
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-0.1.0-blue" alt="Version"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="MIT licence"></a>
</div>

<!-- template:begin -->
This repository is a starter for building a Rust command-line app, with an example, local data storage and checks for every change.

This template's CLI is pre-1.0.
<!-- copy: A Rust command-line app for [what it helps you do]. -->
<!-- copy: This project is pre-1.0. -->
<!-- template:end -->

<!-- crates:begin -->
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

<!-- template:begin -->
## Starting point

Create a copy with GitHub's **Use this template**, clone it, then run `scripts/init.sh <name> [--kind cli|cli-bare|workspace] [--repository <url>]` and `make ci`:

- `cli` (the default) keeps the full template with its `note` example.
- `cli-bare` keeps the layers with one read-only `status` command and no example domain.
- `workspace` keeps an empty Cargo workspace with the gates, CI, lints, agent guide and design-doc templates.

The script renames the project, removes what the kind does not keep, writes `template.toml` (the template version and the kind) and deletes itself. [`template.toml`](template.toml) here holds the template version; a change to a shipped file raises it.
<!-- copy: ## Starting point -->
<!-- copy: Generated from the constellation Rust CLI template, version __VERSION__, kind `__KIND__`; [`template.toml`](template.toml) records both. -->
<!-- copy: Its template baseline and kind are recorded in [`template.toml`](template.toml). -->
<!-- template:end -->

## Learn more

- [Documentation index](docs/INDEX.md) for the documentation home.

<!-- crates:begin -->
- [User guide](docs/usage.md) for commands and troubleshooting.
- [Configuration](docs/usage.md#configuration) for flags, environment variables and defaults.
<!-- crates:end -->
- [Architecture](ARCHITECTURE.md) for the crate layout and dependency rules.
- [Feature board](docs/design/FEATURES.md) for design documents and their delivery stages.
- [Changelog](CHANGELOG.md) for release history.

## Contributing

See the [contributing guide](CONTRIBUTING.md) for the development gates and test workflow, and [SECURITY.md](SECURITY.md) to report a security problem.

This repository follows the constellation standards vendored in [`docs/standards/`](docs/standards/). Outside the constellation umbrella the standards are plain vendored files that `docs/standards/check.sh` only verifies.

## Licence

This project uses the [MIT License](LICENSE).
