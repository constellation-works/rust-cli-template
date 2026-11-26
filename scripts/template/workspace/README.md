<div align="center">
  <h1>tmpl-cli</h1>
  <p>One line: what tmpl-cli does and for whom.</p>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-0.1.0-blue" alt="Version"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="MIT licence"></a>
</div>

<!-- template:begin -->
<!-- copy: ## Starting point -->
<!-- copy: -->
<!-- copy: Generated from the constellation Rust CLI template, version __VERSION__, kind `__KIND__`; [`template.toml`](template.toml) records both. -->
<!-- template:end -->

## Quick start

Prerequisites: Rust (via rustup), Git, Make, Python 3, `jq`, and a checkout of this repository.

```sh
make ci
```

Cargo targets skip until the workspace has its first crate; see [Architecture](ARCHITECTURE.md) to add one.

## Learn more

- [Architecture](ARCHITECTURE.md) for the crate layout and dependency rules.
- [Feature board](docs/design/FEATURES.md) for design documents and their delivery stages.
- [Changelog](CHANGELOG.md) for release history.

## Contributing

See the [contributing guide](CONTRIBUTING.md) for the development gates and test workflow, and [SECURITY.md](SECURITY.md) to report a security problem.

This repository follows the constellation standards vendored in [`docs/standards/`](docs/standards/). Outside the constellation umbrella the standards are plain vendored files that `docs/standards/check.sh` only verifies.

## Licence

This project uses the [MIT License](LICENSE).
