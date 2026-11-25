---
title: tmpl-cli — Decisions
owner: claude
last_updated: 2026-11-25
last_validated: 2026-09-26
status: Draft
feature: tmpl-cli
doc_role: decisions
type: design
summary: Why three crates laid out by app over the cw-kit (binary, core with apps, runtime and composition, store with a driver, repositories and a contract module; the one error type, file access and the SQLite notes database from the kit's cw-base, cw-fs and cw-sqlite), why commands are one module per noun, why mutating commands are audited through one guard, why --json is the only alias, why a list's JSON is an envelope, why std file locks, why clap has no color, why the notes live in SQLite and how its schema evolves, how the store stays private, and every recorded standard deviation or rule the template does not exercise.
tags: [tmpl-cli, decisions]
paths: ["Cargo.toml", "crates/**", "Makefile"]
related_features: [tmpl-cli]
---

# tmpl-cli — Decisions

Record non-obvious decisions here by title. Superseded decisions remain in place so their original reasoning stays legible. See [CONVENTIONS.md §3](../CONVENTIONS.md#3-what-earns-a-decision-entry) for the admission rule and required `Cost:` line.

## A domain library crate and a CLI crate

**Superseded** by [The template is laid out by app in three crates](#the-template-is-laid-out-by-app-in-three-crates).

**Recorded:** 2026-09
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

**Superseded** by [The template is laid out by app in three crates](#the-template-is-laid-out-by-app-in-three-crates).

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
**Code anchors:** `crates/tmpl-cli/src/command/mod.rs`, `crates/tmpl-cli/src/command/note.rs`

### Context

A single `command.rs` dispatching every verb, with every flag in `cli.rs`,
reads well at three verbs and badly at thirty: each new noun edits both
files, and the flags sit far from the handlers that use them.

### Decision

`command/mod.rs` holds the `Command` enum of nouns, the audited-command
table and `run_with`, the one chokepoint. Each noun is a module holding its
subcommand enum, its `Args` structs, help examples and handlers
(`command/note.rs`); `cli.rs` keeps only the program root and the global
flags. A noun that outgrows one file becomes a directory split along its
verbs, `command/note/{mod,add,list,show,support}.rs` with `tests/` beside
them, as Orbit's `command/` tree does. The split follows responsibilities, not
length (STD-02@3 §R18).

### Consequences

- Adding a noun is one new module plus one variant and one `run` arm.
- Cost: clap declarations are spread across modules, so the whole command
  tree is no longer readable in one file; `--help` goldens stay the review
  surface for it.
- Update (2026-11-24): the audited-command table and the chokepoint moved
  into the app (`Command::audit` and `exec` in
  `crates/tmpl-cli-core/src/app/note/mod.rs`); a noun's module now maps its
  arguments to the app's `Command` and its `Outcome` to a payload. See
  [The template is laid out by app in three crates](#the-template-is-laid-out-by-app-in-three-crates).

## Mutating commands are audited through one guard

**Recorded:** 2026-09-26
**Code anchors:** `crates/tmpl-cli-core/src/runtime/audit.rs::AuditGuard`; `crates/tmpl-cli-core/src/runtime/exec.rs::exec`; `crates/tmpl-cli-core/src/app/note/mod.rs::Command::audit`

### Context

A store changed by several people, scripts and agents needs an answer to
"what changed it, and did that work". Logging from each handler repeats the
rule per verb and misses early returns and panics. Auditing every command
would make `list` and `show` write, which STD-01@2 §R31 forbids and which
would break them on a read-only data directory.

### Decision

Orbit's pattern: an RAII `AuditGuard` created at the dispatch chokepoint
(STD-02@3 §R24), starting in the failure state, told the outcome by
`mark_result`, writing one JSON line to `<root>/audit.jsonl` on drop after
`output::finish` flushes the command's payload or error object.
`Command::audit` declares per command, with an exhaustive match, whether it
is audited; read-only commands return `None`. A line holds identifiers only
(actor, command path, note id, status, stable error code, timing), so no free
text is persisted and no redactor is needed (STD-05@1 §R13). The log is a side
channel and fails open with a warning (STD-02@3 §R31). Appends hold their own
`.audit.lock` (separate from `.lock`), so concurrent audit lines never
interleave and a writer that timed out on `.lock` still records its failure
line within one deadline.

### Consequences

- A panic or early return in a handler is logged as a failure.
- Update (2026-11-24): the guard lives in the core's runtime and the
  chokepoint is the runtime's `exec`, which every app's `exec` forwards to.
  It takes the surface's `finish` so the line is still written after the
  payload or error object is flushed.
- Cost: every audited command pays a second lock and an `fsync`; a command
  that cannot write its line still succeeds, so the log can miss entries,
  and it grows without rotation.

## --json is the one alias for --format json

**Recorded:** 2026-09
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

**Recorded:** 2026-09
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

**Recorded:** 2026-09
**Code anchors:** `crates/tmpl-cli-store/src/store.rs::Store::lock`; the kit's `cw_fs::Lock::acquire`

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
- Update (2026-11-24): the lock is now the kit's `cw_fs::Lock`, on the same
  std primitive (see [The foundation is the cw-kit](#the-foundation-is-the-cw-kit)).
  Its poll starts at 1 ms and doubles to 20 ms; the holder record is one
  JSON line, and a timeout names the holder only while that process is
  alive. The same thread asking again for a lock it holds is refused at
  once (`store_lock_reentrant`) instead of waiting out its deadline.
- Update (2026-11-25): only the audit log takes this lock now. Note writes
  are SQLite transactions, which wait on SQLite's own lock (see
  [The notes are a SQLite database](#the-notes-are-a-sqlite-database)), and
  a note-write timeout names no holder.

## clap without color or terminal-width features

**Recorded:** 2026-09
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

- Piped help is byte-stable for the goldens. The output sink supplies a
  terminal width for wrapping root command summaries.
- Cost: help is plain text and flag descriptions keep clap's unwrapped layout.

## Usage errors keep clap's rendering

**Recorded:** 2026-09
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
- Update: STD-01@2 §R19 allows exactly this rendering, so it is
  no longer a deviation.
- Deviation from STD-08@2 §R10. Human usage errors retain clap's tips and
  usage line. Command errors include the resolved note ID or path when
  those details explain the failure, as required by STD-02 §R26.

## standards-check warns locally and fails in CI

**Recorded:** 2026-09
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

**Recorded:** 2026-09
**Code anchors:** `crates/tmpl-cli/src/output/payload.rs::ListPayload`; `crates/tmpl-cli-core/src/app/note/rule.rs::NoteList`; `crates/tmpl-cli/src/command/note.rs::list_notice`

### Context

`note list --limit N` can return fewer notes than matched. STD-01@2 §R34
requires a capped list to say so in its machine payload, and warns that
turning a shipped bare array into an envelope later is a breaking change
(§R10). The alternative, dropping `--limit` so a bare array complies, only
postpones that break to the first project that needs a limit.

### Decision

`note list --json` is always `{"notes": [...], "total": N, "truncated":
bool}`, whether or not a limit applied, and an empty result is the same
envelope with `"notes": []` (§R16). The note app filters before it limits
(§R33) and reports the match count; a cut list also prints `showing N of M
matching notes` on stderr in every mode. `--limit` must be at least 1.

### Consequences

- A consumer never branches on flags or counts to find the records, and a
  capped page cannot pass for a complete one.
- Cost: `jq '.notes[]'` instead of `jq '.[]'`, and every future list verb
  inherits the envelope.

## add names what it wrote on stderr

**Recorded:** 2026-09
**Code anchors:** `crates/tmpl-cli/src/command/note.rs::output`; `crates/tmpl-cli-core/src/app/note/add.rs::add`

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

**Recorded:** 2026-09
**Code anchors:** `crates/tmpl-cli-store/src/db/format.rs::UPGRADES`, `::FORMAT`

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
- Update (2026-11-25): for the notes this registry is replaced by the
  database's append-only `MIGRATIONS` in `driver/sqlite/schema/` (see
  [The notes are a SQLite database](#the-notes-are-a-sqlite-database)); the
  rule is the same, keyed on `PRAGMA user_version`. The audit line keeps
  its own `format` and `UPGRADES` in `db/audit.rs`.

## Store state is owner-only and checked on every load

**Recorded:** 2026-09
**Code anchors:** `crates/tmpl-cli-store/src/store.rs::Store::write`, `::Store::load`; the kit's `cw_fs::create_private_dir`, `cw_fs::inspect_dir`, `cw_fs::inspect_file`, `cw_fs::read`, `cw_fs::Lock::acquire`

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
- Update (2026-11-24): the checks are now the kit's `cw-fs` (see
  [The foundation is the cw-kit](#the-foundation-is-the-cw-kit)), which is
  stricter in four ways. Any group or other permission bit is refused, not
  only a write bit, so a `0644` store or `0755` data directory is refused
  too. The data directory may no longer be a symbolic link (a link in a
  component above it is still followed): whoever can replace the link can
  point the data anywhere. A file is checked again after it is opened, and a
  store or lock file that is not a regular file, or a lock file with a
  second hard link, is refused (`store_not_regular`). A store file larger
  than 64 MiB is refused unread (`store_too_large`). The remedy now reads
  `chmod -- 600 '<path>'`, safe to paste for any file name. The template
  depends on `rustix` no longer; the kit does.
- Update (2026-11-25): the notes database and its `-wal` and `-shm` are
  checked and created by the kit's `cw-sqlite`, through `cw-fs`, the same
  way; the store still checks the data directory itself before every read
  and write. The 64 MiB cap went with the JSON document: SQLite reads pages,
  not the whole file.

## One malformed note makes the store corrupt

**Superseded** by [Bad note records are isolated on read and block writes](#bad-note-records-are-isolated-on-read-and-block-writes).

**Recorded:** 2026-09
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

## Bad note records are isolated on read and block writes

**Recorded:** 2026-11-21
**Code anchors:** `crates/tmpl-cli-store/src/db/format.rs::decode`, `crates/tmpl-cli-store/src/store.rs::Store::write`, `crates/tmpl-cli-store/src/repository/note.rs::Notes`

### Context

Validating `Title` and `Tag` inside serde made one hand-edited note render the
entire store unreadable, while `#[serde(default)]` silently fabricated missing
fields (`body`, `tags`, `priority`) that the next write persisted. STD-02@3
§R32 asks for per-record isolation on reads, while STD-02@3 §R16 and §R31
forbid silently dropping or defaulting damaged records on write.

### Decision

`db/format.rs::decode` parses the top-level document and then validates
each note entry at the construction boundary without `#[serde(default)]`.
Valid notes are returned alongside one typed `BadRecord` per invalid entry
(naming its id, or 1-based line number when the id is unreadable, and the
reason). Read commands (`list`, `show` of a good id) succeed with the valid
notes and emit one stderr warning per bad record in every mode. `show` of a
bad id fails with `store_bad_record`. Every write command refuses while any
bad record is present (`store_bad_record`), leaving `notes.json` untouched.

### Consequences

- Good notes stay readable even when one entry was damaged by hand, and a
  damaged store is never rewritten with dropped or defaulted records.
- Cost: reads carry the bad records beside the notes (today the notes
  repository's `bad_records` and the app's `Outcome`), and `format.rs`
  validates note fields explicitly rather than deriving `Deserialize` on
  `Note`.
- Update (2026-11-24): the write refusal is enforced twice: the note app's
  `add` refuses on its own read before any side effect, and `Store::write`
  refuses again under the lock, since a rewrite would drop the bad records.
- Update (2026-11-25): on SQLite each row is mapped to the contract types in
  `repository/note.rs`; the table's `STRICT` types, `NOT NULL` columns,
  primary key and `json_valid` check leave a bad value in a valid column
  (a title, tag or priority a contract refuses, an unparseable timestamp) as
  the remaining failure, and a bad record is located by its id. A write no
  longer rewrites the other rows, so it could not drop a bad one; the
  refusal stays, so a bad record is repaired before notes are added beside
  it and the commands behave as before. Duplicate ids can no longer be
  stored, so `store_duplicate_id` is gone.

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

## Environment settings fall back rather than refuse

**Recorded:** 2026-09
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

**Recorded:** 2026-09

### Context

The [adopted standards](../../standards/README.md) are pinned to the canon.
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
  there is nothing to skip (STD-04@2 §R8).
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

## The template is laid out by app in three crates

**Recorded:** 2026-11-24
**Code anchors:** `Cargo.toml` `[workspace] members`; `scripts/check-dependency-direction.sh::policy`; `crates/tmpl-cli-core/src/runtime/exec.rs::App`, `::exec`; `crates/tmpl-cli-core/src/app/note/mod.rs::exec`; `crates/tmpl-cli-store/src/store.rs::Store`

### Context

Daniel, 2026-11-24: "can you also update rust-cli-template similarly?
separate create for store. runtime, composition, apps in core." "Similarly"
means the layout he approved for scribe the same day: every command is
parsed by the binary, run by one `exec` in the core, and rendered by the
binary; everything about one subject is in one app folder; persistence is
its own crate with one repository per record and the record shapes as its
contracts.

The two-crate template had the failures that layout removes, at small scale.
The binary held composition (`app.rs`) and the audit guard, so a second
surface would have had to copy both. The core mixed the note rules
(`note.rs`, `query.rs`) with record shapes in one module, and its `store`
module named its operations after commands (`add`, `list`, `show`) and held
the next-id rule and list selection inside the facade. A project grown from
the template would copy those habits.

### Decision

Three crates, `tmpl-cli -> tmpl-cli-core -> tmpl-cli-store`
([ARCHITECTURE.md](../../../ARCHITECTURE.md)):

- `tmpl-cli` declares arguments, maps them to an app's `Command`, calls
  `app::<app>::exec` and renders the `Outcome`; `main` captures the
  environment into the core's `Env` once. It names only the core.
- `tmpl-cli-core` holds `app/<app>/` (`Command`, `Outcome`, `Error`,
  `exec`, `run`, one flow per command and the app's pure `rule`),
  `runtime/` (the context a flow receives, the one chokepoint, the audit
  guard, the errors every app shares) and `composition.rs` (the only code that reads the
  environment or picks the data directory, the store and the clock). A flow
  loads from one `store.read()`, decides with its rules and commits in one
  `store.write(|tx| …)`.
- `tmpl-cli-store` holds `db/` (fsio, the persisted document and its
  upgrades, the audit line, locks; `fsio` later moved to the foundation), `repository/` (notes, audit) and
  `contract/` (`Note`, `NoteId`, `Title`, `Body`, `Tag`, `Priority`,
  `AuditEvent`). It holds no rule of the note app.

Two departures from scribe's layout, both from the template's size: no
`tools` crate and no `providers`, since nothing here reads transcripts or
calls a model. One from the plain `exec(env, command)` shape: `exec` also
takes the surface's `finish`, because the audit line must be written after
the payload or error object is flushed (the integration test
`json_error_object_precedes_audit_failure_warning_on_stderr` holds that
order). Runtime start, the audit guard and the `finish` order are written once, in
the runtime's generic `exec`; an app's `exec` is a one-line forward and the
app supplies only what differs through the `App` trait (whether a command is
audited, its dispatch, the record id and error code for the audit line).
Daniel, 2026-11-24, on the first draft, where each app's `exec` held the
wrapper: "this is being called for each app group. perhaps keeping the
composition at the choke point was a better idea?" A project grown from the
template would have pasted the wrapper into every app, and one could have
left the guard out. STD-02@4 §R7 asks a new crate for a dependency edge that
must be enforceable; the store crate is one: the core cannot reach a file primitive
or the persisted format, only the repositories.

### Consequences

- A new subject is a new app folder in the core, a repository and a contract
  in the store if it persists something, and a noun module in the binary;
  nothing else moves.
- A second surface (an MCP server, a desktop app) calls the same `exec` and
  gets composition and auditing with it.
- A new app implements `App` and forwards its `exec`; it cannot run a
  command outside the guard, and `scripts/check-dependency-direction.sh`
  rejects an app that starts the runtime or the guard itself.
- Tests that need a pinned clock or a short lock wait run an app through the
  core's `test-support` feature (`Ctx`, `note_run`) instead of `exec`.
- Cost: three manifests; record shapes are `pub` across a crate boundary;
  STD-02@4 §R5's I/O-free contract unit is the `contract` module, kept free
  of I/O by grep, inside a crate that does I/O, which is weaker than a crate
  edge; and `exec` carries a `finish` callback instead of returning the
  outcome.
- Update (2026-11-24): the error type and the file mechanism (`db/fsio.rs`)
  moved to a fourth crate, a foundation crate, so the store names no file
  primitive. The record shapes stay in the store. See
  [One error type and file access live in a foundation crate](#one-error-type-and-file-access-live-in-a-foundation-crate).
  The same day the foundation crate was replaced by the kit's `cw-base` and
  `cw-fs`, leaving three crates again: see
  [The foundation is the cw-kit](#the-foundation-is-the-cw-kit).

## One error type and file access live in a foundation crate

**Superseded** by [The foundation is the cw-kit](#the-foundation-is-the-cw-kit):
the crate this decision made is deleted, and its two folders are the kit's
`cw-base` and `cw-fs`. The reasoning below stands; the anchors name what
replaced the deleted files.

**Recorded:** 2026-11-24
**Code anchors:** the kit's `cw_base::Error`, `cw_base::Kind` and `cw_fs::Error`; `crates/tmpl-cli-store/src/error.rs` (`impl From<Error> for cw_base::Error`); `crates/tmpl-cli-core/src/app/note/mod.rs::Error`; `crates/tmpl-cli/src/error.rs::exit_code`; `scripts/check-dependency-direction.sh::policy`

### Context

With three crates the errors nested: the store's `Error` inside a runtime
`Error` inside each app's `Error` inside the binary's `CliError`. `?`
converts one step, so every app wrote a `From<store::Error>` by hand to
cross two, the binary matched an app's variant to choose a remedy, and each
app's `exec` returned a different error type. Daniel, 2026-11-24, on that
hand-written conversion: "what is this"; then: "we are going to make the
best version possible now. This repo will serve as the basis for all future
projects, so we need to get the first footing right. what do you think about
creating a leaf node, [the common crate], and we can port the global errors
there". While it was built: "no flat layers in the common crate. any common
types/contracts can go there too"; "fs utils can go there in common"; and,
after the record shapes had been moved there: "actually, contracts can stay
in store - core is importing store anyways".

The typed enums cannot simply move down: the runtime's error wrapped the
store's, so a crate holding it would depend on the store and not be a leaf,
and the store's variants (corrupt, bad record, newer format) are the
store's own vocabulary. A single enum of every unit's variants in the leaf,
as Orbit's `OrbitError` is, would make every new app edit the foundation.

### Decision

A fourth crate, the foundation (named `common`), that names no other crate
of the workspace. It is grouped by subject, one folder each:

- `error/`: one `Error` for the whole workspace (STD-02@4 §R11). It wraps a
  unit's typed error whole and adds a `Kind` and the stable code. Its
  `Display` and source chain are the typed error's own. `Kind` names what
  the caller can do: `NotFound`, `Refused`, `Busy`, `Corrupt`,
  `Incompatible`, `Config`, `Io`, `Internal`.
- `fs/`: file access for a private data directory (atomic write, append,
  advisory lock, owner-only checks), moved from the store's `db/fsio.rs`,
  with its own typed `Error`. It knows no record and no file name, and it is
  the only code in the workspace that names `std::fs`. The store wraps its
  error as `Error::Fs` and gives each variant the store's stable code, so
  no code changed.

The record shapes (`contract/`) stay in the store: the core imports the
store already, so a lower home would serve no unit that cannot reach them.

Each unit keeps its typed `thiserror` enum for its own matching (§R10) and
has one translator, a `From` impl beside the enum, that states the kind and
the code (§R12, the case where the owning unit can name the surface type).
The store's `Error`, composition's `Error` (no data directory) and the note
app's `Error` each have one. An app's enum is its own refusals plus one
`Store(#[from] …)` variant; the runtime's error enum is gone. Every app's
`exec` returns `Result<Outcome, Error>` with the foundation's `Error`, so the runtime's
`App` trait asks an app for no error code, and the audit line reads the
code from the one type. `run` still returns the app's typed error.

The binary has no error enum. It decides the exit code from `Kind` in one
exhaustive match, keeps the remedy text (spelled in its own commands) keyed
on the app's typed error through `downcast_ref`, and builds its one failure
(output that could not be written) as the same type. It still names only
the core, which re-exports `Error`, `Kind` and the store's `contract`.

Three readings of the standard are deliberate. STD-02@4 §R10 says library
code never returns `Box<dyn Error>`: `Error` holds one inside a typed
struct, the typed enum stays intact behind it, and classification is by
`Kind` and code, never by message. §R11 asks `#[non_exhaustive]` of the
surface error type: `Error` carries it; `Kind` and `fs::Error` do not, so
that adding a kind or a file failure breaks every match that must decide
about it. §R7 starts a CLI at two crates and asks a dependency reason for
each further one: the store and the core must both name the error type, and
the store cannot depend on the core, so it has to sit below both.

### Consequences

- A new app writes no error conversion by hand, and a surface handles one
  type whatever the app.
- "Only one place touches a file" is a crate fact for the store and the
  core: neither depends on a file crate, and the guard rejects `std::fs` in
  both. A later unit that owns a file of its own uses the same mechanism
  without depending on the store.
- The guard keeps the foundation grouped by subject (no file beside
  `lib.rs`, no folder named for no responsibility) and keeps `error/` free
  of I/O and of the clock.
- STD-02@4 §R5's I/O-free contract unit is still the store's `contract`
  module, kept so by grep; the store crate itself now names no file
  primitive either.
- Cost: four manifests. A surface matches on `Kind` and code instead of an
  app's variants, and reaches a variant only by `downcast_ref`. A test
  through `exec` asserts on the code; a test through `run` keeps the typed
  enum. `error/` has no unit test of its own: `Error` is exercised through
  the binary's error tests and goldens.
- Cost: today only the store uses `fs/`, so it is in the foundation for the
  projects grown from this template, not for a second user here.
- Cost: `common` is a name for no responsibility. A later version of the
  standard than this repository pins (STD-02@6 §R43) allows that name in
  the foundation only. What keeps it from collecting code is its section in
  [ARCHITECTURE.md](../../../ARCHITECTURE.md) and the guard, not its name.

## The foundation is the cw-kit

**Recorded:** 2026-11-24
**Code anchors:** `Cargo.toml` `[workspace.dependencies]` `cw-base`, `cw-fs`, `cw-testkit`; `crates/tmpl-cli/tests/support/mod.rs::Fixture`; `crates/tmpl-cli/tests/golden.rs::goldens`; `deny.toml` `[sources] allow-git`; `crates/tmpl-cli-store/src/error.rs::Error::code`; `crates/tmpl-cli-store/src/db/mod.rs::STORE_CAP`; `crates/tmpl-cli/src/error.rs::exit_code`; `scripts/check-dependency-direction.sh::policy`

### Context

The foundation crate held mechanism every constellation Rust CLI needs and
no project-specific rule: one error type and file access for a private
data directory. Each project grown from the template would carry its own
copy and drift. The cw-kit design moves that mechanism into versioned
crates in the public
[constellation-works/satellites](https://github.com/constellation-works/satellites)
repository, and the template is its first adopter, so its CI is the kit's
integration test.

### Decision

The workspace depends on the kit's `cw-base` (`Error`, `Kind`, a clock) and
`cw-fs` (atomic write, append, bounded read, listing, removal, private
directories, owner-only checks, one advisory lock) as git dependencies
pinned to commit `9c9cc998720f0a75041be3451c14fbaf80f29dc8`, since no release
is published; `deny.toml` allows that one git source. The foundation crate
is deleted, and the template keeps three crates.

- The store and the core name `cw-base`; only the store names `cw-fs`; the
  binary names neither and reaches `Error` and `Kind` through the core's
  re-export. `scripts/check-dependency-direction.sh` holds each crate's kit
  list and fails a `cw-*` dependency outside it.
- Each unit keeps its typed `thiserror` enum and its one `From` impl into
  `cw_base::Error`, naming the `Kind` and the template's own stable code.
  What crosses a crate boundary is `cw_base::Error`: the store's public
  operations return it, so the core never sees the store's typed error
  except by `downcast_ref`, and a note app's store failure is
  `Store(cw_base::Error)`.
  `cw_fs::Error` carries a kind and no code: the store wraps it as
  `Error::Fs` and names a code for every variant. The existing codes are
  unchanged; the kit's three further refusals get `store_not_regular`,
  `store_too_large` and `store_lock_reentrant`.
- Every store read states a cap (64 MiB, `STORE_CAP`), which the foundation's
  read did not, and every write checks the encoded document against the same
  cap before it replaces the file, so a write never leaves a store its own
  reads refuse (STD-02@4 §R16). Recovery for a store over the limit is in
  [specs/store.md](./specs/store.md).
- `Kind` gained `Usage`. clap still reports today's usage errors before a
  command runs; the binary's exit-code match maps `Usage` to 2, as
  STD-01@2 §R20 asks.

### Consequences

- A fix to the mechanism lands once in the kit and reaches the template by
  moving the pin; a project grown from the template inherits the pin and
  moves it on its own schedule.
- The kit is stricter than the foundation was; what changed for an operator
  is listed under
  [Store state is owner-only and checked on every load](#store-state-is-owner-only-and-checked-on-every-load).
- Update (2026-11-24): the tests take the kit's `cw-testkit`, a
  dev-dependency of every crate at the same rev: the fixture's temp root and
  isolated child are `TempRoot` and `TempRoot::command`, its timestamp
  redaction is `redact_timestamps`, and the goldens compare and rewrite
  through `Goldens` (`UPDATE_GOLDENS=1`, so `make goldens UPDATE=1` is
  unchanged). What stays the template's is the binary's name, the seeded
  notes and the store files written by hand. A child now also gets `PATH`
  and `GIT_CEILING_DIRECTORIES`, and runs in the temp root.
- Update (2026-11-25): the store also depends on `cw-sqlite` at the same
  rev, for the notes database (see
  [The notes are a SQLite database](#the-notes-are-a-sqlite-database)). The
  64 MiB read cap and the write's cap check went with the JSON document.
- Cost: a build fetches the satellites repository once; an unpublished pin
  is a commit hash a reader must look up; and a kit change the template
  needs waits for a satellites change and a pin move.

## The notes are a SQLite database

**Recorded:** 2026-11-25
**Code anchors:** `crates/tmpl-cli-store/src/driver/sqlite/mod.rs::open`; `crates/tmpl-cli-store/src/driver/sqlite/schema/mod.rs::MIGRATIONS`; `crates/tmpl-cli-store/src/driver/sqlite/schema/note.rs::CREATE_NOTE`; `crates/tmpl-cli-store/src/repository/note.rs`; `crates/tmpl-cli-store/src/store.rs::Store::read`, `::Store::write`; `crates/tmpl-cli-store/src/contract/note.rs::NoteId::MAX`

### Context

Every `add` rewrote the whole JSON document under a file lock, a reader had
to parse all of it, and a format change locked older builds out of every
field. The kit now has `cw-sqlite`: one private SQLite database per tool,
WAL, one `IMMEDIATE` write transaction, a read-only reader, owner-only files
and an append-only list of migrations. The cw-kit design settles that the
example store moves onto it and that the audit log stays an append-only file.

### Decision

The notes live in `<root>/notes.db`, opened only through `cw_sqlite::Db`.

- **A driver sets one storage mechanism up once and returns a handle; a
  repository is one record's use of it.** `driver/sqlite/mod.rs` holds the
  file name and the lock wait and returns the `Db`; it runs no query. Its
  `schema/` holds one module per table, exporting that table's steps as
  named items, and `schema/mod.rs` names every step in one ordered list. `repository/note.rs` holds the notes' SQL and the mapping
  between a row and the contract types, run on the reader `Store::read`
  opened or the transaction `Store::write` began. There is no `driver/fs/`:
  the audit repository uses `cw_fs` directly, and the audit files move with
  the `cw-app` adoption.
- **The schema is an append-only list of steps** from version 1, with a
  `STRICT` table, RFC 3339 UTC text timestamps and the tags as a JSON array
  checked by `json_valid`. A statement that changes the schema appears only
  in `driver/sqlite/schema/`. The list names each step rather than joining
  per-table lists, so a later step for an earlier table goes at the end and
  no shipped step is renumbered (STD-03 R23).
- **The record-access seam.** The core reaches a record only through
  `snapshot.<record>()` and `tx.<record>()`. The store's public surface has
  no `rusqlite` or `cw_sqlite` type, no borrow from a connection or a
  document, and no path but `Store::path()` and `Store::audit_path()`, each
  with a caller in the core. Repository calls return owned values and are
  fallible, since each runs a query. There is no trait per repository and no
  driver chosen at run time; when a second driver is added,
  `repository/note.rs` becomes `repository/note/{mod.rs, sqlite.rs,
  <other>.rs}` ([ARCHITECTURE.md](../../../ARCHITECTURE.md)).
- `rusqlite` is named only inside `tmpl-cli-store`, through `cw_sqlite`'s
  re-export. Another crate's test that needs a store the public API never
  writes (a bad record, the last id, a newer schema) runs raw SQL through the
  store's `test-support` feature. `scripts/check-dependency-direction.sh`
  enforces these rules where source text can show them.
- Each `cw_sqlite::Error` the store can meet gets a template code
  (`store_locked`, `store_corrupt`, `store_too_new`, the file codes and the
  rest, listed in [specs/store.md](./specs/store.md)). The codes the JSON
  document had are kept where the failure still exists.
- `NoteId` is bounded at `i64::MAX`, SQLite's largest integer, so
  `note_id_exhausted` is reached there.
- No import of a JSON store: the example app has no deployed data.

### Consequences

- A write changes one row in one transaction instead of rewriting every
  note, and a reader sees one committed version without a lock.
- The schema evolves as appended steps, like the format registry did.
- Cost: the database is not a text file to read or repair by hand; a bad row
  is fixed with SQL. `cw-sqlite` bundles SQLite, so a cold build compiles
  it. A note-write timeout names no holder. A note id above `i64::MAX` is now
  a usage error (exit 2) where it was a missing note (exit 1). SQLite keeps
  `-wal` and `-shm` beside the database, and in a writable directory a read
  may rebuild the `-shm` index and creates either when it is missing.
- Update (2026-11-25): `db/` is gone. It held only the audit line's
  encoding, its upgrades and `AUDIT_LOCK_FILE`, which are the audit record's
  mapping, so they live with the audit repository as the note's SQL lives in
  `repository/note.rs`: `repository/audit/mod.rs` holds the append and
  `AUDIT_LOCK_FILE`, and `repository/audit/line.rs` the persisted line and
  `UPGRADES`, still naming neither `cw_fs` nor `cw_sqlite`.
  `scripts/check-dependency-direction.sh` fails if `db/` comes back.

## Public design documents use public references

**Recorded:** 2026-11

### Context

STD-06@1 §R6 and §R13 describe how design records cite tasks. The template
is public and the Constellation tracker is private.

### Decision

Describe the next step in the feature board and use public issues, pull
requests or repository paths when a design needs supporting references.
A design without public task references omits the task references section.
The tracked-file gate enforces this policy for private Orbit IDs.

### Consequences

- The public references resolve for readers of the repository.
- Cost: readers need access to Orbit for private tracker provenance.

## Rules the template does not yet meet after the 829a01d re-vendor

**Recorded:** 2026-11-25

### Context

The template adopts STD-12@1 in the change that adds its gates. The
standards sync tool vendors the whole adopted set from one umbrella commit,
so the same sync at 829a01d also moved five standards on:

- STD-01 from 2 to 4;
- STD-02 from 4 to 8;
- STD-06 from 1 to 2;
- STD-08 from 2 to 3;
- STD-09 from 1 to 3.

The template was audited against each rule that is new or changed between
the versions it vendored before and the ones it vendors now. STD-12@1 is met
in full by the gates added with it.

### Decision

Each rule below is a recorded deviation until the follow-up that brings the
template into line closes it. That follow-up is not part of the change that
re-vendors. Each line names the rule and what is unmet.

- **STD-01@4 §R25**:
  - the environment variables are not one documented constant list stating
    each one's meaning, default and the flag it overrides;
  - `NO_COLOR`, `CLICOLOR_FORCE`, `TERM`, `COLUMNS`, `HOME`, `USER` and
    `USERNAME` are read as bare literals;
  - `docs/usage.md`'s table is written by hand rather than generated;
  - the help text types `$TMPL_CLI_ROOT` and `$TMPL_CLI_FORMAT` by hand.
- **STD-02@8 §R6**:
  - `scripts/check-dependency-direction.sh` has no module import-cycle check;
  - `clippy.toml` carries none of the `disallowed-types` or
    `disallowed-methods` baseline;
  - the script's grep bans on `std::fs`, `std::net`, `std::process` and
    `std::env` stand where Clippy is to take them over.
- **STD-02@8 §R7**: the binary crate reads the environment, for the
  presentation variables and the log filter, in `tmpl-cli/src/output/`.
- **STD-02@8 §R39**: the store's top-level modules import each other in a
  cycle: `store.rs` imports `repository::Audit`, and `repository/audit/mod.rs`
  imports `Store` back.
- **STD-02@8 §R44**: the format-1 audit-line upgrader (`UPGRADES` in
  `repository/audit/line.rs`) lives in the live audit codec, which the append
  path in `repository/audit/mod.rs` imports, rather than in a unit named for
  upgrades.
- **STD-02@8 §R43**: `make structure` has no unit-map gate, which would fail
  on a crate or top-level module missing from `ARCHITECTURE.md`, a mapped path
  that does not exist, or a unit with no `//!` doc.
- **STD-06@2 §R4**: the numbered design docs and the doc templates in
  `docs/design/_templates/` carry `last_updated`, not `created_date` and
  `updated_date`.
- **STD-09@3 §R15**: some docs lack `type` and `summary` frontmatter:
  - `ARCHITECTURE.md`, `CHANGELOG.md`, `about.md`, `docs/usage.md`,
    `docs/development.md`;
  - `docs/design/CONVENTIONS.md`, `docs/design/FEATURES.md` and
    `docs/design/_templates/1_scope.md`.
- **STD-09@3 §R16**: there is no generated `docs/INDEX.md` and no CI check
  that it is current.
- **STD-09@3 §R17**: no link and anchor check covers the root and `docs/`
  Markdown; `scripts/check-readme.py` checks `README.md` only.
- **STD-09@3 §R23**: there is no `docs/architecture/README.md` and no page
  per crate.
- **STD-09@3 §R25**: there is no `SECURITY.md`.
- **STD-09@3 §R26**: there is no root `CONTRIBUTING.md`. `docs/development.md`
  holds the setup and checks, and says how agents may be used only through a
  link to `AGENTS.md`.

### Consequences

- A reviewer checks a change against the vendored versions, and finds each
  gap above already recorded rather than introduced by the change.
- Cost: until the follow-up lands, a project made from the template inherits
  these gaps along with the record of them.
