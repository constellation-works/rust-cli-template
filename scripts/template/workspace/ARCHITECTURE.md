# Architecture

The layer model that `scripts/check-dependency-direction.sh` enforces
(STD-02 R1, R6). A lower layer never imports a higher one. Adding a crate or
an edge changes this file and that script in the same commit. Every crate
and top-level module has its place here, and every path written in code
formatting exists (`scripts/check-architecture-map.sh`, STD-02 R43).

## Crates

The workspace has no crate yet. `make ci` runs the gates that need no build
and says it skips the Cargo targets until the first `crates/*/Cargo.toml`
exists.

A new crate goes under `crates/` and gets:

- a member entry in the root `Cargo.toml` (`[workspace] members`);
- a row in the table below, with what it owns, its internal and kit
  dependencies, and what it must never depend on;
- a section of its own in this file, headed with the crate's name in code
  formatting, naming each top-level module, and a first-line `//!` doc on
  the crate root and each top-level module
  (`scripts/check-architecture-map.sh` fails until both are there);
- a policy entry in `scripts/check-dependency-direction.sh`, without which
  the check fails.

| Crate | Kind | Owns | Internal and kit dependencies | Banned |
|-------|------|------|-------------------------------|--------|

The cw-kit (`cw-base`, `cw-fs`, `cw-sqlite`, `cw-app`, `cw-cli`, with `cw-testkit` as
a dev-dependency only) from
[constellation-works/satellites](https://github.com/constellation-works/satellites)
is pinned to one commit in the root `Cargo.toml` `[workspace.dependencies]`.
Each crate's README there is its contract.

## Module rules

- Every crate: module, source directory and source file names are singular
  (`scripts/check-singular-names.sh`, STD-12 R1), and the source style that
  stable rustfmt cannot check holds (`scripts/check-source-style.sh`, STD-12
  R12, R13, R16, R19).
- Unit tests live in a sibling `tests/` directory mirroring source file
  names (`scripts/check-test-modules.sh`, STD-02 R19, R20).
- Only `crates/tmpl-cli/src/output/` touches stdout, stderr or the terminal
  (`scripts/check-terminal-guard.sh`): a binary crate named `tmpl-cli` keeps
  its output there.
