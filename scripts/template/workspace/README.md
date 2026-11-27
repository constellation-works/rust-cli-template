<div align="center">
  <h1>tmpl-cli</h1>
<!-- template:begin -->
  <p>A blank workspace for a new constellation software project</p>
<!-- copy:   <p>A project workspace starter.</p> -->
<!-- template:end -->
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-0.1.0-blue" alt="Version"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="MIT licence"></a>
</div>

<!-- template:begin -->
An empty Rust workspace with documentation, quality gates and CI ready for a first crate.

This template workspace is pre-1.0.
<!-- copy: A starting workspace for [what this project will do]. -->
<!-- copy: This project is pre-1.0. -->
<!-- template:end -->

## Quick start

Prerequisites: Rust (via rustup), Git, Make, Python 3, `jq`, and a checkout of this repository.

```sh
make ci
```

Cargo targets skip until the workspace has its first crate; see [Architecture](ARCHITECTURE.md) to add one.

<!-- template:begin -->
## Starting point

The workspace starts empty. Add a crate to the root Cargo workspace, then describe its place in ARCHITECTURE.md and scripts/check-dependency-direction.sh.

This workspace follows the constellation standards in [`docs/standards/`](docs/standards/). Outside the constellation umbrella, [`docs/standards/check.sh`](docs/standards/check.sh) only verifies the vendored files.

<!-- copy: ## Starting point -->
<!-- copy: This workspace starts empty. Add a crate to Cargo.toml and record its place in ARCHITECTURE.md and scripts/check-dependency-direction.sh. -->
<!-- copy: This workspace follows the constellation standards in [`docs/standards/`](docs/standards/). Outside the constellation umbrella, [`docs/standards/check.sh`](docs/standards/check.sh) only verifies the vendored files. -->
<!-- copy: Generated from the constellation Rust CLI template, version __VERSION__, kind `__KIND__`; [`template.toml`](template.toml) records both. -->
<!-- template:end -->

## Learn more

- [Documentation index](docs/INDEX.md) for the documentation home.
- [Architecture](ARCHITECTURE.md) for the crate layout and dependency rules.
- [Feature board](docs/design/FEATURES.md) for design documents and their delivery stages.
- [Changelog](CHANGELOG.md) for release history.

## Contributing

See the [contributing guide](CONTRIBUTING.md) for the development gates and test workflow, and [SECURITY.md](SECURITY.md) to report a security problem.

## Licence

This project uses the [MIT License](LICENSE).
