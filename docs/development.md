# Contributing

Install Rust with rustfmt and Clippy, Git, Make, Python 3 and `jq` (the
dependency-direction gate reads `cargo metadata` with it). The
[CI workflow](../.github/workflows/ci.yml) pins the Rust toolchain,
cargo-deny and cargo-nextest versions.

```sh
make ci-fast          # formatting, structure and source style, docs and standards
make ci-lint          # Clippy and cargo-deny
make test             # nextest and doctests
make goldens          # compare help and output from the built binary
make ci               # run all gates
```

After an intended help or output change, run `make goldens UPDATE=1`
and review the diff. After a design stage or next step changes, run
`make feature-board` and include the updated board.

The [agent guide](../AGENTS.md) describes branch and workspace rules.
The [architecture](../ARCHITECTURE.md) describes the layers. Copy the
appropriate design scaffold from [the design conventions](design/CONVENTIONS.md)
when a new feature needs a design.

Locally, the Makefile warns about missing cargo-deny and cargo-nextest and
uses plain Cargo tests when nextest is unavailable. CI requires both tools.
