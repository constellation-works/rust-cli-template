---
title: Kit — Decisions
owner: claude
created_date: 2026-11-27
updated_date: 2026-11-27
last_validated: 2026-11-27
status: Accepted
feature: kit
doc_role: decisions
type: design
summary: Each standard deviation and each rule left unexercised that the code and gate scripts of every kind of copy still carry, moved here from the example's decisions so that no kind loses the record.
tags: [kit, decisions]
paths: ["Makefile", "scripts/**", "crates/**"]
related_features: [kit]
---

# Kit — Decisions

Record a deviation here when the code or the gate script that carries it ships
in more than one kind of copy. A decision about the crates sits in a crates
region, which the `workspace` kind drops; the other decisions reach every kind.
A deviation that only the example carries stays in the example's decisions.
`scripts/check-template.py` holds each title below to the kinds that carry it.
See [CONVENTIONS.md §3](../CONVENTIONS.md#3-what-earns-a-decision-entry) for
the admission rule and the required `Cost:` line.

## Missing gate tools warn locally and fail in CI

**Recorded:** 2026-09
**Code anchors:** `Makefile` `missing_tool`, `test`, `goldens`, `deny`, `ci-fast`

### Context

STD-04@2 §R10 says a gate whose tool is missing fails, and §R12 puts every
parity check CI runs into the fast local gate, except those that need a
build. `cargo-nextest` (per-test timeouts, `--no-tests=fail`) and
`cargo-deny` are not installed everywhere a copy of this template is built.

### Decision

Deviation from STD-04@2 §R10 for local runs only: without `cargo-deny` the
supply-chain check is skipped, and without `cargo-nextest` the tests run
under `cargo test`, each with a warning naming the install command. Under
`CI=true` (set by GitHub Actions) either missing tool fails the gate, and CI
installs both at pinned versions. `ci-fast` runs formatting, the structure
checks and `standards-check`. Two checks stay out of it under STD-04@2 §R12
because they need a build: the help and output goldens (`make goldens`) and
Cargo.lock freshness (`--locked` on every cargo build).

### Consequences

- A fresh clone passes `make ci` with only cargo installed, and CI cannot
  pass with a gate missing.
- Cost: a local green `make ci` without those tools is weaker than CI's.

## standards-check warns locally and fails in CI

**Recorded:** 2026-09
**Code anchors:** `Makefile` `standards-check`; `.github/workflows/ci.yml` `STANDARDS_STRICT`

### Context

The adopted standards are vendored into `docs/standards/` by the
constellation's `sync-standards.sh`, which may run after the repository is
created. A fresh copy must still pass `make ci`, but a real project whose
standards were never vendored should not pass CI. STD-04@2 §R10 asks a gate
whose tool is missing to fail.

### Decision

Deviation from STD-04@2 §R10 for local runs only: without
`docs/standards/check.sh`, `make standards-check` prints a warning and the
command to run, and exits 0 — unless `STANDARDS_STRICT=1`, which CI sets, and
then it fails.

### Consequences

- The template and a just-created project pass `make ci` locally.
- Cost: a local `make ci` can pass while CI fails, until the standards are
  vendored.

## The vendored standards carry no project frontmatter

**Recorded:** 2026-11-25
**Code anchors:** `scripts/check-doc-frontmatter.py::documents`

### Context

STD-09@4 §R15 asks every Markdown document under `docs/` to open with
`type` and `summary`, and §R29 asks each such document for `created_date`
and `updated_date`. The vendored standards under `docs/standards/` are
Markdown under `docs/`, and most of them carry no such frontmatter (the
`STD-nn.md` files carry the canon's own). STD-09 also says an adopting repo
never edits its vendored copy, and `docs/standards/check.sh` fails on any
byte changed there.

### Decision

Deviation from STD-09@4 §R15 and §R29 for `docs/standards/` only:
`scripts/check-doc-frontmatter.py` skips the directory, and the template
adds no frontmatter to it. The alternative, adding frontmatter to the
vendored files, breaks the vendoring check and the rule never to edit them.

### Consequences

- Every other document under `docs/` is held to both rules.
- Cost: the vendored files stay outside the frontmatter check until the
  canon either carries the frontmatter itself or exempts vendored copies.

<!-- crates:begin -->
## Environment settings fall back rather than refuse

**Recorded:** 2026-09
**Code anchors:** the kit's `cw_cli::Terminal::decide` and `cw_cli::Logging::from_env`; `crates/tmpl-cli/src/lib.rs::run`

### Context

STD-02@11 §R28 refuses a bad configuration value at load, naming the key.
STD-01@4 §R8 requires an unrecognized output-format variable to fall back
to `auto`, so one exported typo does not break every command in a shell.
The log filter is a side channel (STD-02@11 §R31).

### Decision

Deviation from STD-02@11 §R28 for the environment: an unrecognized
`TMPL_CLI_FORMAT` means `auto`, and a `TMPL_CLI_LOG` that does not parse is
reported once on stderr and replaced by `warn`. Command-line values are
validated when parsed (a numeric bound, a non-blank text) and refused as
usage errors naming the flag.

### Consequences

- A stray variable never stops a command.
- Cost: an unrecognized `TMPL_CLI_FORMAT` gives no warning, by STD-01's
  rule.

## Usage errors keep clap's tips and usage line

**Recorded:** 2026-09
**Code anchors:** `crates/tmpl-cli/src/output/common/error.rs::finish_parse_error`

### Context

STD-08@4 §R10 asks an error shown to a reader to say in one plain line what
failed, why and the next step. clap's usage errors are an `error:` line
followed by a tip or did-you-mean suggestion, the usage line and a pointer to
`--help`. STD-01@4 §R19 allows exactly this multi-line rendering for the
form of the error; STD-08 still reads it as more than one line.

### Decision

Deviation from STD-08@4 §R10: human usage errors retain clap's tips and
usage line, written after the first `error: …` line as a notice (exit 2). In
JSON mode a usage error is one `{"error","code":"usage_error"}` object like
every other error. Command errors include the resolved record ID or path when
those details explain the failure, as STD-02@11 §R26 requires.

### Consequences

- Wrappers still branch on exit code 2 and the stable `code`.
- Cost: a script reading human-mode stderr gets several lines for a usage
  error and one line for a command failure.

## cw_base::Error holds a boxed cause and carries no non_exhaustive

**Recorded:** 2026-11-24
**Code anchors:** the kit's `cw_base::Error`, `cw_base::Kind`; `crates/tmpl-cli-store/src/error.rs` (`impl From<Error> for cw_base::Error`); `crates/tmpl-cli/src/error.rs`

### Context

STD-02@11 §R10 says library code never returns `Box<dyn Error>` and that a
failure is classified from structure, never from a message. §R11 asks the
surface error type to be `#[non_exhaustive]` when it is part of a public API.
The store and the core both return the kit's `cw_base::Error`, which wraps a
unit's typed error in a boxed cause beside a `Kind` and a stable code, and
`Kind` is a plain enum.

### Decision

Three readings of the standard are deliberate. §R10: `cw_base::Error` holds
the box inside a typed struct built only through `Error::new` from the unit's
one translator, each unit keeps its typed `thiserror` enum intact behind it,
and a caller classifies by `Kind` and code or by `downcast_ref` to the typed
enum, never by message. §R11: `Error` has private fields and one constructor,
so it cannot be built or matched field by field from outside; `Kind` is not
`#[non_exhaustive]`, so that adding a kind breaks every match that must decide
about it, and the binary maps `Kind` to an exit code in one exhaustive match.
§R12: each unit translates with one `From` impl beside its enum.

### Consequences

- A new unit writes one translator, and a surface handles one type whatever
  the unit.
- Cost: a surface reaches a variant only by `downcast_ref`, and a new `Kind`
  is a breaking change for every surface that matches on it.

## Owner-only checks are no-ops without Unix modes

**Recorded:** 2026-09
**Code anchors:** the kit's `cw_fs::create_private_dir`, `cw_fs::inspect_dir`, `cw_fs::inspect_file`; `crates/tmpl-cli-store/src/store.rs::Store::write`, `Store::read`

### Context

STD-05@1 §R8 wants state created owner-only regardless of the umask, and §R9
wants it refused when it is foreign-owned, writable by others, or reached
through a link. std has no safe way to get the effective uid, and Windows has
no mode bits; STD-05 says a platform without file modes meets both rules with
owner-only ACLs.

### Decision

Deviation from STD-05@1 §R8 and §R9 for platforms without Unix modes: the
data directory is created `0700` and the files `0600`, and every load refuses
foreign ownership, a group or other permission bit and a link, on Unix. On
other platforms the kit's `cw-fs` creates and checks nothing: its owner and
mode check returns success and its files are opened with the default access
list. Setting and checking owner-only ACLs would need a Windows security API
the kit does not take.

### Consequences

- Unix, where CI runs the template, meets both rules in full.
- Cost: on Windows a store is as private as its parent directory's access
  list makes it, and nothing refuses a store another user can read.
<!-- crates:end -->

## Standard rules this template does not exercise

**Recorded:** 2026-09

### Context

The [adopted standards](../../standards/README.md) are pinned to the canon.
Some of their rules govern things this template's code does not do. A project
grown from the template must not read their absence as a waiver.

### Decision

These rules are met trivially today because the code does not do what they
govern. Each binds as soon as it does, and this entry is edited then:

<!-- template:begin -->
<!-- copy-workspace: - A workspace copy has no crate yet, so every rule about code a crate runs binds when the first crate is added. -->
<!-- template:end -->
<!-- crates:begin -->
- No destructive verb, prompt, removed flag or default filter: STD-01@4 §R5,
  §R27, §R35 (and §R33's default-filter clause).
- No agent or script hand-off, no recovery path: STD-02@11 §R25, §R33.
- No async code, channels or nested locks in the product: STD-03@2 §R1–§R3.
  Tests use bounded channels and deadlines for thread and child waits (§R2,
  §R17, §R22). One store file, so no multi-store commit or replay: §R8, §R9.
- No spawned or detached processes in the product: STD-03@2 §R11–§R16 and
  STD-05@1 §R10–§R12. Test fixtures that spawn the binary follow STD-03@2
  §R17–§R20.
- No installed defaults, long-lived processes, moving references,
  integrity guards, deletion, remote calls or git automation: STD-03@2
  §R25–§R32.
- No capability-dependent tests: every test runs on every CI runner, so
  there is nothing to skip (STD-04@2 §R8).
- No authorization, plugins, credentials, listeners, consent or tokens:
  STD-05@1 §R3, §R4, §R15–§R19, §R21, §R22, §R25. The binary makes no
  network request at all (§R20). Its one identity is the audit line's
  `actor`, which the kit reads from `USER`, else `USERNAME`: a claim, not an
  identity the OS vouches for. §R1 and §R2 apply to it as a claim: it is
  stored in its own field and never used as authority.
- No persisted free text except content the caller asked to store, kept
  verbatim, so there is no redaction layer (STD-05@1 §R13, §R14). The audit
  log persists identifiers and stable codes only; the integration tests pin
  its fields, and adding a free-text field to it brings §R13 into force.
<!-- crates:end -->

### Consequences

- A reviewer checks a new feature against this list first.
- Cost: the list goes stale unless the change that starts doing one of
  these things also edits it.
