---
type: guide
summary: How to set up a checkout of tmpl-cli, the checks a change must pass, and how agents may work on it.
created_date: 2026-11-22
updated_date: 2026-11-25
---

# Contributing

## Setup

Install Rust through rustup, Git, Make, Python 3 and `jq` (the
dependency-direction gate reads `cargo metadata` with it). Rustup installs
the toolchain that [`rust-toolchain.toml`](rust-toolchain.toml) pins, with
rustfmt and Clippy. The [CI workflow](.github/workflows/ci.yml) reads that
pin and pins the cargo-deny and cargo-nextest versions.

## Required checks

A pull request targets `agent-main` and must pass `make ci` on Linux and
macOS in CI. Before handing a change over, run at least:

```sh
make ci-fast          # formatting, structure and source style, docs and standards
make ci-lint          # Clippy and cargo-deny
make test             # nextest and doctests
make goldens          # compare help and output from the built binary
make ci               # run all gates
```

After an intended help or output change, run `make goldens UPDATE=1`
and review the diff. After a design stage or next step changes, run
`make feature-board` and include the updated board. After adding,
removing or re-summarizing a document, run `make doc-index` and include
the updated [index](docs/INDEX.md).

The [architecture](ARCHITECTURE.md) describes the layers. Copy the
appropriate design scaffold from [the design conventions](docs/design/CONVENTIONS.md)
when a new feature needs a design.

Locally, the Makefile warns about missing cargo-deny and cargo-nextest and
uses plain Cargo tests when nextest is unavailable. CI requires both tools.

## Using agents

Coding agents may write and review changes here. The
[agent guide](AGENTS.md) (also `CLAUDE.md`) is their brief: the rules a
change follows, the branch rules, and the gates. A change made with an
agent meets the same checks and review as any other, and a person decides
what merges: neither an implementation request nor a passing PR authorizes
an agent to merge. Agents never put an internal task or record id in help,
messages or a public document.
<!-- template:begin -->

The template's own gates: `scripts/check-template.py` (part of
`make structure`) keeps the `note` example removable, as
[`scripts/template-manifest.txt`](scripts/template-manifest.txt) lists
it, and `scripts/check-template-version.sh <base>` fails a change to a
shipped file that does not raise `version` in `template.toml`. The
template workflow runs `scripts/init.sh` for each kind on a scratch copy,
then `make ci` there.
<!-- template:end -->
