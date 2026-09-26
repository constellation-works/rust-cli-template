---
title: tmpl-cli — Design
owner: claude
last_updated: 2026-09-26
last_validated: 2026-09-26
status: Draft
feature: tmpl-cli
doc_role: design
type: design
summary: How tmpl-cli is layered, how an invocation flows from argv to bytes, how the store stays consistent, and which gates hold each standard in place.
tags: [tmpl-cli, cli, architecture]
paths: ["crates/**", "scripts/**", "Makefile"]
related_features: [tmpl-cli]
related_artifacts: [ORB-13117, ORB-13134]
---

# tmpl-cli — Design

The current implementation: layering, the life of one invocation, the store,
and the gates. The output and store contracts are specified in
[specs/output-contract.md](./specs/output-contract.md) and
[specs/store.md](./specs/store.md); open directions are in
[3_vision.md](./3_vision.md).

## 1. Layering

Two crates (why: [4_decisions.md](./4_decisions.md#a-domain-library-crate-and-a-cli-crate)).
`tmpl-cli-core` holds contract types (`note`), mechanisms (`fsio`), and the
domain (`store`, `error`); it has no clap, no terminal access and never reads
the environment. `tmpl-cli` holds composition (`app`) and the surface (`cli`,
`command`, `output`). The tier table and module rules are in
[ARCHITECTURE.md](../../../ARCHITECTURE.md), and
`scripts/check-dependency-direction.sh` enforces both from source text alone,
so it runs in `make ci-fast` without compiling.

## 2. The life of an invocation

1. `main` parses argv with `Cli::parse_args`. `--help`/`--version` go to
   stdout (exit 0); a usage error goes to stderr (exit 2) — as one JSON
   object when the raw arguments or `TMPL_CLI_FORMAT` asked for JSON.
2. `Sink::from_process` resolves the output mode (flag > `TMPL_CLI_FORMAT`
   > auto), color (`NO_COLOR`, `CLICOLOR_FORCE`, `TERM`, TTY) and width
   (`COLUMNS`, terminals only). Nothing else in the program asks these
   questions; `scripts/check-terminal-guard.sh` rejects any attempt.
3. `app::Context::resolve` picks the data directory and constructs the store.
4. `command::run` makes one store call and wraps the result in a payload
   (`Output::Notes` or `Output::Note`) with an optional stderr notice. Core
   errors cross into `CliError` through its single `#[from]`.
5. `output::finish` renders the payload for the sink, or reports the error
   (`error: …` line, or `{"error","code"}` in JSON mode) and returns exit 1.
   A write that fails with `BrokenPipe` ends the process with exit 0 and no
   message.

stdout carries only the payload. Notices (nothing matched, the list was cut,
`added note <id> to <file>`), errors and `tracing` logs (`TMPL_CLI_LOG`,
default `warn`; an unparseable filter is reported and ignored) go to stderr.

## 3. Rendering

Every mode renders the same `NotePayload`. `table` is borderless, one header
row, two-space gutters, the ID column right-aligned, absent cells as `-`,
priority colored by role (high → warn, low → muted) when color is allowed.
On a terminal with a known width the TITLE column shrinks and ends in `…`;
`note show` and `--json` always carry the full title. `plain` (auto in a
pipe) is one tab-separated line per record without a header. `json` is one
pretty-printed document; for a list it is always the envelope `{"notes",
"total", "truncated"}`. A single-record list is still a table; an empty list
prints nothing (or the envelope with no notes) plus one stderr line naming
the filters. Filters apply before `--limit`, and a cut list says so on stderr
in every mode.

## 4. The store

`notes.json` holds `{"format": 1, "notes": [...]}`. Reads take no lock and
write nothing: the file is only ever replaced by rename, so readers see a
whole version, and `list`/`show` work on a read-only data directory. Every
load first checks that the data directory and the store file belong to this
user and are not writable by others, and that the store file is not a link;
otherwise it refuses with the `chmod` that fixes it. `add` loads once before
touching anything, so a refused store leaves nothing behind; it then creates
the data directory (`0700`), takes `.lock` (`0600`, never through a link; an
OS advisory lock with a 10-second deadline, and a timeout names the holder's
pid, label and start time), re-reads under the lock, assigns max id + 1, and
writes through `fsio::write_atomic` (a `0600` temp file in the same
directory, `fsync`, rename, `fsync` of the directory). The temp file removes
itself on every failure path. A file declaring a newer format is refused for
reads and writes; an older one is upgraded in memory through the append-only
`UPGRADES` registry (empty so far) and persisted by the next write.

## 5. Gates

| Gate | Make target | Holds |
|------|-------------|-------|
| `cargo fmt --check` | `ci-fast` | formatting |
| `check-dependency-direction.sh` | `ci-fast` | crate and module layering (STD-02 R1–R6), no clock-defaulted timestamps (R16) |
| `check-test-modules.sh` | `ci-fast` | every sibling unit-test file is declared (STD-02 R19) |
| `check-terminal-guard.sh` | `ci-fast` | one owner for streams, TTY and color env (STD-01 R12, R17) |
| `docs/standards/check.sh` | `ci-fast` (`standards-check`) | vendored standards (warns until vendored; fails in CI) |
| `clippy --all-targets -D warnings` with the STD-02 lint table and its adoptable additions | `ci-lint` | no unwrap/expect/print/dbg, no lock across await, no wildcard enum arms, no narrowing `as` |
| `cargo deny check` | `ci-lint` | advisories, licenses, sources |
| nextest `--no-tests=fail` (timeouts in `.config/nextest.toml`, no retries), then `cargo test --doc` | `test` | behaviour, and runnable doc examples |
| help and output goldens from the built binary | `goldens` | the public surface (STD-01 R24) |

Every cargo invocation passes `--locked`. Locally a missing `cargo-deny` or
`cargo-nextest` warns and the gate skips or falls back; under `CI=true` it
fails. CI runs `make ci` on Linux and macOS with a 30-minute job timeout,
with actions pinned by commit SHA and the toolchain and tools by version;
Dependabot proposes updates to both crate and action pins.

## 6. Concerns & Honest Limitations

- Terminal width comes only from `COLUMNS`, which most shells set but do not
  export, so truncation rarely engages; querying the terminal needs a
  dependency (`terminal_size`) the template does not take. Wide (CJK, emoji)
  characters are counted as one column each.
- Usage errors keep clap's multi-line rendering in human mode
  ([4_decisions.md](./4_decisions.md#usage-errors-keep-claps-rendering)).
- The whole store is one file rewritten on every `add`: fine for thousands of
  notes, wrong for millions. That is the domain's problem to outgrow, not the
  template's.
- Under plain `cargo test` (nextest absent) there is no per-test timeout;
  only the CI job timeout bounds a hang.
- Holder information in `.lock` is best effort: it is written just after the
  lock is taken, so a waiter can briefly read the previous holder's line, and
  a failure to write it is logged rather than failing the write.
- The ownership and mode checks are unix-only; on Windows they are no-ops.
- One unparseable note makes the whole store `store_corrupt`
  ([4_decisions.md](./4_decisions.md#one-malformed-note-makes-the-store-corrupt)).

## Task References

- [ORB-13117] — built the Rust CLI template this project was created from.
- [ORB-13134] — brought the template to STD-01..03 v2 and STD-04/05 v1.

> Resolve any task above with `orbit task show <ID>` or `git log --grep=<ID>`.
