---
title: tmpl-cli — Decisions
owner: claude
last_updated: 2026-09-26
last_validated: 2026-09-26
status: Draft
feature: tmpl-cli
doc_role: decisions
type: design
summary: Why two crates with persistence as one core module, why commands are one module per noun, why mutating commands are audited through one guard, why --json is the only alias, why a list's JSON is an envelope, why std file locks, why clap has no color, how the store format evolves and stays private, and every recorded standard deviation or rule the template does not exercise.
tags: [tmpl-cli, decisions]
paths: ["Cargo.toml", "crates/**", "Makefile"]
related_features: [tmpl-cli]
related_artifacts: [ORB-13117, ORB-13134]
---

# tmpl-cli — Decisions

Record non-obvious decisions here by title. Task references carry provenance; superseded decisions remain in place so their original reasoning stays legible. See [CONVENTIONS.md §3](../CONVENTIONS.md#3-what-earns-a-decision-entry) for the admission rule and required `Cost:` line.

## A domain library crate and a CLI crate

**Recorded:** 2026-09 · [ORB-13117]
**Code anchors:** `Cargo.toml` `[workspace] members`; `scripts/check-dependency-direction.sh::policy`

### Context

STD-02@1 §R7 said to start as one crate and split only for a build or
dependency need (STD-02@2 §R7 has since made two crates the default for a new
CLI, for the reason below). A single crate can express the layers as modules with grep
bans. The template, though, exists to be copied by agents into projects that
grow, and the edge that matters most — the domain must not know about
argument parsing, terminals or log subscribers — is exactly "a dependency
edge that must be enforceable", R7's own justification for a split.

### Decision

Two crates: `tmpl-cli-core` (library; business logic and persistence) and
`tmpl-cli` (binary; composition and surface). Cargo makes clap unreachable
from the core rather than merely banned, and the core is ready to be consumed
by a second surface (MCP server, desktop app) without restructuring. Further
crates still need an R7 reason; the default for new code is a module in one
of these two. A project that is certain to stay a single small binary may
collapse to one crate with STD-02's single-crate check.

### Consequences

- The domain/surface boundary is enforced by the compiler plus
  `check-dependency-direction.sh`, not by review.
- Cost: two manifests, a `[workspace.dependencies]` table, and a crate
  boundary that forces `pub` on the core's API where one crate would use
  `pub(crate)`.

## Persistence is one module inside the core crate

**Recorded:** 2026-09-26
**Code anchors:** `crates/tmpl-cli-core/src/store/mod.rs`; `scripts/check-dependency-direction.sh` (the `store | tests` loop)

### Context

A core crate that mixes business rules with file handling becomes hard to
extend: the rules get tested through a filesystem, and a second backend (a
database, a remote store) means untangling them first. The obvious fix is
more crates (types, domain, storage, CLI), but four crates is heavy for a
starting template, and STD-02@3 §R7 asks a further crate for a build or
dependency reason the template does not have yet.

### Decision

Keep two crates, and split the core by responsibility inside it. Business
logic (`note`: types and the next-id rule; `query`: list selection) does no
I/O and never imports `crate::store`. Everything that touches disk is the
`store` module: the `Store` facade over private submodules `format`, `audit`
and `fsio`, declared with `pub(super)` so nothing outside `store` can reach a
file primitive. The dependency-direction script bans file and process access,
and `crate::store` imports, in every other core module, including ones added
later. When persistence earns its own crate (a second backend, or heavy
dependencies the rules should not compile against), `store/` moves out whole.

### Consequences

- The selection and id rules are unit-tested without a filesystem.
- Cost: `Store` stays a concrete type, not a trait, so swapping the backend
  still edits the facade; the module boundary is enforced by visibility and
  grep, which are weaker than a crate edge.

## Commands are one module per noun

**Recorded:** 2026-09-26
**Code anchors:** `crates/tmpl-cli/src/commands/mod.rs`, `crates/tmpl-cli/src/commands/note.rs`

### Context

A single `command.rs` dispatching every verb, with every flag in `cli.rs`,
reads well at three verbs and badly at thirty: each new noun edits both
files, and the flags sit far from the handlers that use them.

### Decision

`commands/mod.rs` holds the `Command` enum of nouns, the audited-command
table and `run`, the one chokepoint. Each noun is a module holding its
subcommand enum, its `Args` structs, help examples and handlers
(`commands/note.rs`); `cli.rs` keeps only the program root and the global
flags. A noun that outgrows one file becomes a directory split along its
verbs, `commands/note/{mod,add,list,show,support}.rs` with `tests/` beside
them, as Orbit's `command/` tree does. The split follows responsibilities, not
length (STD-02@3 §R18).

### Consequences

- Adding a noun is one new module plus one variant and one `run` arm.
- Cost: clap declarations are spread across modules, so the whole command
  tree is no longer readable in one file; `--help` goldens stay the review
  surface for it.

## Mutating commands are audited through one guard

**Recorded:** 2026-09-26
**Code anchors:** `crates/tmpl-cli/src/audit_middleware.rs::AuditGuard`; `crates/tmpl-cli/src/commands/mod.rs::Command::audit`, `::run`

### Context

A store changed by several people, scripts and agents needs an answer to
"what changed it, and did that work". Logging from each handler repeats the
rule per verb and misses early returns and panics. Auditing every command
would make `list` and `show` write, which STD-01@2 §R31 forbids and which
would break them on a read-only data directory.

### Decision

Orbit's pattern: an RAII `AuditGuard` created at the dispatch chokepoint
(STD-02@3 §R24), starting in the failure state, told the outcome by
`mark_result`, writing one JSON line to `<root>/audit.jsonl` on drop.
`Command::audit` declares per command, with an exhaustive match, whether it
is audited; read-only commands return `None`. A line holds identifiers only
(command path, note id, status, stable error code, timing), so no free text
is persisted and no redactor is needed (STD-05@1 §R13). The log is a side
channel and fails open with a warning (STD-02@3 §R31). Appends hold the store
lock, so concurrent commands never interleave lines.

### Consequences

- A panic or early return in a handler is logged as a failure.
- Cost: every audited command pays a second lock and an `fsync`; a command
  that cannot write its line still succeeds, so the log can miss entries,
  and it grows without rotation.

## --json is the one alias for --format json

**Recorded:** 2026-09 · [ORB-13117]
**Code anchors:** `crates/tmpl-cli/src/cli.rs::GlobalArgs`, `crates/tmpl-cli/src/cli.rs::Cli::parse_args`

### Context

STD-01 R7 requires at least `--json` on every data command; R4 wants the
output mode as one global flag and R3 one spelling per concept. `--format`
also has to exist for `table` and `auto`. clap's `conflicts_with` does not
see a global flag given at a different subcommand level, so
`tmpl-cli --json note list --format table` would parse silently.

### Decision

`--json` is a global boolean documented as shorthand for `--format json`;
it is the only alias, and no other output flag may be added. The conflict is
checked after parsing in `Cli::parse_args` and reported as a clap usage error
(exit 2).

### Consequences

- Scripts can rely on `--json` everywhere, humans on `--format`.
- Cost: two spellings of one mode, and a hand-written conflict check that a
  new mode flag must extend.

## An empty JSON list is []

**Superseded** by [A list's JSON is an envelope with total and truncated](#a-lists-json-is-an-envelope-with-total-and-truncated).

**Recorded:** 2026-09 · [ORB-13117]
**Code anchors:** `crates/tmpl-cli/src/output/render.rs::note_list`

### Context

STD-01 R16 says a query that matches nothing prints nothing to stdout; R7
says JSON mode emits one JSON document, an array for a list. An empty stdout
is not a JSON document, so `--json | jq` would fail on an empty result.

### Decision

In JSON mode an empty list prints `[]`; in table and plain modes it prints
nothing. In every mode the one-line notice goes to stderr and the exit code
is 0. This reads R16's "nothing" as "no records", as Orbit does.

### Consequences

- `jq` and `wc -l` both behave on empty results.
- Cost: JSON consumers and line consumers see different bytes for the same
  empty result, which the goldens and tests must pin separately.

## The store lock uses std file locks, not a crate

**Recorded:** 2026-09 · [ORB-13117]
**Code anchors:** `crates/tmpl-cli-core/src/store/fsio.rs::LockGuard::acquire`

### Context

STD-03 R6 needs a kernel advisory lock released on holder death. Rust 1.89
stabilized `File::lock`/`try_lock` (`flock` on unix, `LockFileEx` on
Windows); before that, constellation code used `fs4`.

### Decision

Use `std::fs::File::try_lock` with an in-process polling deadline, and raise
`rust-version` to 1.89 for it. Prefer std over a crate whenever std covers
the need.

### Consequences

- One fewer dependency to audit; the lock semantics are std's documented ones.
- Cost: the minimum supported Rust is 1.89, and there is no blocking-with-
  timeout primitive, so the wait is a 20 ms poll.

## clap without color or terminal-width features

**Recorded:** 2026-09 · [ORB-13117]
**Code anchors:** `Cargo.toml` `[workspace.dependencies] clap`

### Context

STD-01 R17 requires the color/TTY decision in exactly one place. clap's
default `color` feature and the `wrap_help` feature each query the terminal
themselves, and a terminal-dependent help rendering would also make the help
goldens depend on where they were captured.

### Decision

clap is built with `default-features = false` and without `color` or
`wrap_help`. Help and usage errors are plain text everywhere; styling, if it
is ever wanted, is applied from the sink's decision.

### Consequences

- Help renders byte-identically on every machine, so goldens are stable.
- Cost: no colored help and no wrapping of long help lines to the terminal.

## Usage errors keep clap's rendering

**Recorded:** 2026-09 · [ORB-13117]
**Code anchors:** `crates/tmpl-cli/src/output/mod.rs::finish_parse_error`

### Context

STD-01@1 §R19 says a non-JSON error is a single `error: <message>` line.
clap's usage errors are an `error:` line followed by a tip or did-you-mean
suggestion, the usage line and a pointer to `--help` — the actionable detail
R21 asks for.

### Decision

Deviation from STD-01@1 §R19 for usage errors only: in human mode they keep
clap's multi-line rendering (first line `error: …`, exit 2). In JSON mode they
are one `{"error","code":"usage_error"}` object like every other error.
Command failures follow R19 exactly.

### Consequences

- Wrappers still branch on exit code 2 and the stable `code`.
- Cost: a script reading human-mode stderr gets several lines for a usage
  error and one line for a command failure.
- Update [ORB-13134]: STD-01@2 §R19 allows exactly this rendering, so it is
  no longer a deviation.

## standards-check warns locally and fails in CI

**Recorded:** 2026-09 · [ORB-13117]
**Code anchors:** `Makefile` `standards-check`; `.github/workflows/ci.yml` `STANDARDS_STRICT`

### Context

The adopted standards are vendored into `docs/standards/` by the
constellation's `sync-standards.sh`, which may run after the repository is
created. A fresh copy must still pass `make ci`, but a real project whose
standards were never vendored should not pass CI.

### Decision

Without `docs/standards/check.sh`, `make standards-check` prints a warning
and the command to run, and exits 0 — unless `STANDARDS_STRICT=1`, which CI
sets, and then it fails.

### Consequences

- The template and a just-created project pass `make ci` locally.
- Cost: a local `make ci` can pass while CI fails, until the standards are
  vendored.

## A list's JSON is an envelope with total and truncated

**Recorded:** 2026-09 · [ORB-13134]
**Code anchors:** `crates/tmpl-cli/src/output/payload.rs::ListPayload`; `crates/tmpl-cli-core/src/query.rs::NoteList`; `crates/tmpl-cli/src/commands/note.rs::list_notice`

### Context

`note list --limit N` can return fewer notes than matched. STD-01@2 §R34
requires a capped list to say so in its machine payload, and warns that
turning a shipped bare array into an envelope later is a breaking change
(§R10). The alternative, dropping `--limit` so a bare array complies, only
postpones that break to the first project that needs a limit.

### Decision

`note list --json` is always `{"notes": [...], "total": N, "truncated":
bool}`, whether or not a limit applied, and an empty result is the same
envelope with `"notes": []` (§R16). The store filters before it limits
(§R33) and reports the match count; a cut list also prints `showing N of M
matching notes` on stderr in every mode. `--limit` must be at least 1.

### Consequences

- A consumer never branches on flags or counts to find the records, and a
  capped page cannot pass for a complete one.
- Cost: `jq '.notes[]'` instead of `jq '.[]'`, and every future list verb
  inherits the envelope.

## add names what it wrote on stderr

**Recorded:** 2026-09 · [ORB-13134]
**Code anchors:** `crates/tmpl-cli/src/commands/note.rs::add`

### Context

STD-01@2 §R30 asks a write to name its resolved target in its human output
and its machine payload. The record's `id` is in the payload already. The
store file is what exposes a mis-resolved data directory, but adding it to
the JSON would make `add` and `show` return different shapes.

### Decision

`add` returns the same note payload as `show` (the record, with its `id`) and
prints `added note <id> to <root>/notes.json` on stderr in every mode.
Errors that depend on the data directory name the store file too
(STD-02@2 §R26).

### Consequences

- A human sees where the note went; a script gets the record.
- Cost: the file path is not machine-readable output; a script that needs
  it resolves `--root` itself.

## Store format upgrades are an append-only registry

**Recorded:** 2026-09 · [ORB-13134]
**Code anchors:** `crates/tmpl-cli-core/src/store/format.rs::UPGRADES`, `::FORMAT`

### Context

The store is one JSON document with a `format` number. Adding a defaulted
field without bumping it looks compatible, but an older build then loads the
file, drops the field it does not know and rewrites the store without it,
which is a lossy round trip (STD-02@2 §R16) and an old binary writing newer
state (STD-03@2 §R10). STD-03@2 §R23 wants every layout change shipped as an
appended migration.

### Decision

Every change to the persisted shape bumps `FORMAT` and appends one step to
`UPGRADES` (format `n` document in, `n + 1` out). A shipped step is never
edited, reordered or removed; a unit test holds `FORMAT == UPGRADES.len() +
1`. A newer format is refused for reads and writes. Reads upgrade in memory
only (STD-01@2 §R31); the next `add` persists the current format. The
registry ships empty.

### Consequences

- Fresh and upgraded stores converge on one shape, and older builds refuse
  newer stores instead of damaging them.
- Cost: even an additive field locks older builds out of the store until
  they are upgraded, because a JSON file has no way to open read-only for
  just the fields an old build knows.

## Store state is owner-only and checked on every load

**Recorded:** 2026-09 · [ORB-13134]
**Code anchors:** `crates/tmpl-cli-core/src/store/fsio.rs::create_private_dir`, `::inspect_dir`, `::inspect_file`, `::LockGuard::acquire`

### Context

Notes are personal data. STD-05@1 §R8 wants state created owner-only
regardless of the umask, and §R9 wants it refused or repaired on load when it
is foreign-owned, writable by others, or reached through a link. A repair is
a write, which STD-01@2 §R31 forbids in `list` and `show`. std has no safe
way to get the effective uid, and Windows has no mode bits.

### Decision

The data directory is created `0700` and the store and lock files `0600`,
set explicitly. Every load, reads included, refuses a data directory or
store file that is owned by another uid or writable by group or others,
naming the `chmod` that fixes it (`store_foreign_owner`,
`store_permissions`), and refuses a store or lock file that is a symbolic
link (`store_symlink`). Nothing is repaired automatically. The data
directory itself may be a link, since the operator chooses it; its target is
what gets checked. Because the directory is verified owner-only first, the
files are not re-verified after opening: only the same user could swap them,
and that is not a boundary (STD-05@1 §R5). `rustix` supplies `geteuid` on
unix; on other platforms the checks are no-ops (STD-05@1 §R8, §R9, deviation
for Windows, which would need ACLs).

### Consequences

- Another local user can neither read a new store nor plant one, and a link
  cannot redirect a write.
- Cost: a store created by an older build under a loose umask (`0664`) is
  refused until the operator runs the named `chmod` (STD-03@2 §R24 allows
  this form: the refusal names the file and a repair that needs no tool);
  and one more dependency.

## One malformed note makes the store corrupt

**Recorded:** 2026-09 · [ORB-13134]
**Code anchors:** `crates/tmpl-cli-core/src/store/mod.rs::Store::load`, `crates/tmpl-cli-core/src/store/format.rs::decode`

### Context

STD-02@2 §R32 wants one bad item in a listing isolated and reported while
the rest proceed. The notes live in one document that `add` rewrites whole.
Skipping an unparseable note on read would invite `add` to rewrite the file
without it, which is data loss.

### Decision

Deviation from STD-02@2 §R32: the store file is one unit of integrity. If
any note fails to parse, every command refuses with `store_corrupt`, naming
the file (fail closed, STD-02@2 §R31), and nothing rewrites it. A project
whose records become independent (a file per record, a database) isolates
per record instead.

### Consequences

- A damaged store is never partially rewritten.
- Cost: one bad record hides all the good ones until it is fixed by hand.

## Missing gate tools warn locally and fail in CI

**Recorded:** 2026-09 · [ORB-13134]
**Code anchors:** `Makefile` `missing_tool`, `test`, `goldens`, `deny`, `ci-fast`

### Context

STD-04@1 §R10 says a gate whose tool is missing fails, and §R12 puts every
parity check CI runs into the fast local gate, except those that need a
build. `cargo-nextest` (per-test timeouts, `--no-tests=fail`) and
`cargo-deny` are not installed everywhere a copy of this template is built.

### Decision

Deviation from STD-04@1 §R10 for local runs only: without `cargo-deny` the
supply-chain check is skipped, and without `cargo-nextest` the tests run
under `cargo test`, each with a warning naming the install command. Under
`CI=true` (set by GitHub Actions) either missing tool fails the gate, and CI
installs both at pinned versions. `ci-fast` runs formatting, the structure
checks and `standards-check`. Two checks stay out of it under STD-04@1 §R12
because they need a build: the help and output goldens (`make goldens`) and
Cargo.lock freshness (`--locked` on every cargo build).

### Consequences

- A fresh clone passes `make ci` with only cargo installed, and CI cannot
  pass with a gate missing.
- Cost: a local green `make ci` without those tools is weaker than CI's.

## Environment settings fall back rather than refuse

**Recorded:** 2026-09 · [ORB-13134]
**Code anchors:** `crates/tmpl-cli/src/output/sink.rs::resolve_mode`; `crates/tmpl-cli/src/output/mod.rs::init_logging`

### Context

STD-02@2 §R28 refuses a bad configuration value at load, naming the key.
STD-01@2 §R8 requires an unrecognized output-format variable to fall back
to `auto`, so one exported typo does not break every command in a shell.
The log filter is a side channel (STD-02@2 §R31).

### Decision

Deviation from STD-02@2 §R28 for the environment: an unrecognized
`TMPL_CLI_FORMAT` means `auto`, and a `TMPL_CLI_LOG` that does not parse is
reported once on stderr and replaced by `warn`. Command-line values are
validated when parsed (`--limit` at least 1, a non-blank `--body`) and
refused as usage errors naming the flag.

### Consequences

- A stray variable never stops a command.
- Cost: an unrecognized `TMPL_CLI_FORMAT` gives no warning, by STD-01's
  rule.

## Standard rules this template does not exercise

**Recorded:** 2026-09 · [ORB-13134]

### Context

The template adopts STD-01@2, STD-02@3, STD-03@2, STD-04@1 and STD-05@1.
Some of their rules govern things a three-verb note CLI does not do. A
project grown from the template must not read their absence as a waiver.

### Decision

These rules are met trivially today because the code does not do what they
govern. Each binds as soon as it does, and this entry is edited then:

- No destructive verb, prompt, removed flag or default filter: STD-01@2 §R5,
  §R27, §R35 (and §R33's default-filter clause).
- No agent or script hand-off, no recovery path: STD-02@3 §R25, §R33.
- No async code, channels or nested locks: STD-03@2 §R1–§R3. One store file,
  so no multi-store commit or replay: §R8, §R9.
- No spawned or detached processes in the product: STD-03@2 §R11–§R16 and
  STD-05@1 §R10–§R12. Test fixtures that spawn the binary follow STD-03@2
  §R17–§R20.
- No installed defaults, long-lived processes, moving references,
  integrity guards, deletion, remote calls or git automation: STD-03@2
  §R25–§R32.
- No capability-dependent tests: every test runs on every CI runner, so
  there is nothing to skip (STD-04@1 §R8).
- No authorization, plugins, credentials, listeners, consent or tokens:
  STD-05@1 §R1–§R5, §R15–§R19, §R21, §R22, §R25. The binary makes no
  network request at all (§R20).
- No persisted free text besides note content the caller asked to store,
  kept verbatim, so there is no redaction layer (STD-05@1 §R13, §R14). The
  audit log persists identifiers and stable codes only; a unit test pins its
  fields, and adding a free-text field to it brings §R13 into force.

### Consequences

- A reviewer checks a new feature against this list first.
- Cost: the list goes stale unless the change that starts doing one of
  these things also edits it.

## Task References

- [ORB-13117] — built the Rust CLI template this project was created from.
- [ORB-13134] — brought the template to STD-01..03 v2 and STD-04/05 v1.

> Resolve any task above with `orbit task show <ID>` or `git log --grep=<ID>`.
