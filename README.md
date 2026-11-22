<div align="center">
  <h1>tmpl-cli</h1>
  <p>Keep short notes in a local store</p>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-0.1.0-blue" alt="Version"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="MIT licence"></a>
</div>

<!-- template-only:begin -->
> This repository is the starting point for a new constellation Rust CLI, with two crates, an output contract, a local store, and gates and CI that meet the constellation standards. Create a copy with GitHub's Use this template or the umbrella's operations/scripts/new-rust-cli.sh.
>
> Click **Use this template**, clone the new repository, then run
> `scripts/rename-template.sh <name>` and `make ci`. The rename updates the
> project name, removes this note and deletes the rename script.
<!-- template-only:end -->

<!-- project-summary: One line — what tmpl-cli does and for whom. -->

The example CLI is pre-1.0.

## Quick start

Prerequisites: a Rust toolchain with Cargo and a checkout of this repository.

```sh
cargo install --path crates/tmpl-cli --locked
```

Add a note and see it in the list.

```sh
tmpl-cli note add "Call the plumber" --tag home --priority high
tmpl-cli note list
```

## Learn more

- [User guide](docs/usage.md) for commands and troubleshooting.
- [Configuration](docs/usage.md#configuration) for flags, environment variables and defaults.
- [Architecture](ARCHITECTURE.md) for the crate layout and dependency rules.
- [Feature board](docs/design/FEATURES.md) for design documents and their delivery stages.
- [Changelog](CHANGELOG.md) for release history.

## Contributing

See the [contributing guide](docs/development.md) for the development gates and test workflow.

## Licence

This project uses the [MIT License](LICENSE).
