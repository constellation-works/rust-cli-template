---
title: tmpl-cli — Overview
owner: claude
last_updated: 2026-09-26
last_validated: 2026-09-26
status: Draft
feature: tmpl-cli
doc_role: overview
stage: shipped
next: —
type: design
summary: A small note-keeping CLI whose real job is to be the working example of the constellation CLI, Rust-architecture, process-safety, testing and security standards.
tags: [tmpl-cli, cli, standards]
paths: ["crates/**"]
related_features: [tmpl-cli]
---

# tmpl-cli — Overview

tmpl-cli keeps short notes — a title, optional body, tags and a priority — in
one JSON file under a data directory, through `note add`, `note list` and
`note show`. The domain is deliberately trivial. What matters is the shape
around it: every rule of the constellation standards STD-01 (CLI surface),
STD-02 (Rust architecture and errors), STD-03 (concurrency and process
safety), STD-04 (testing and verification) and STD-05 (security boundaries)
that applies to a small CLI is demonstrated here in code, a test or a gate,
so a project created from this template starts compliant and its agents copy
compliant code. The rules it does not exercise are listed in
[4_decisions.md](./4_decisions.md#standard-rules-this-template-does-not-exercise).

## 1. Motivation

Agents imitate the code around them. A new repository that starts empty gets
whatever structure the first task happens to produce, and the standards
arrive later as review findings against code already written. Starting from a
working, standard-shaped example turns the standards into the path of least
resistance: the next command is a copy of `note list`, the next error a new
variant next to the existing ones, the next test a sibling of the ones already
there.

## 2. Core Concepts

- **Data directory** — where the store lives; resolved once by composition
  (`--root` > `TMPL_CLI_ROOT` > `~/.tmpl-cli`) and handed to the domain.
- **Store** — the SQLite database `notes.db` (with its `-wal` and `-shm`)
  in an owner-only data directory; read through one read-only connection,
  written in one transaction, its schema an append-only list of steps.
- **Payload** — the typed record every output mode renders from; its JSON
  form is the machine contract.
- **Sink** — the per-invocation answer to "who reads stdout": output mode,
  color, width. Resolved once, in one module.

## 3. At a Glance

| Concern | File |
|---------|------|
| Grouped root help | [crates/tmpl-cli/src/help.rs](../../../crates/tmpl-cli/src/help.rs) |
| Root command, global flags, parsing | [crates/tmpl-cli/src/cli.rs](../../../crates/tmpl-cli/src/cli.rs) |
| Environment capture, data directory, store and clock | [crates/tmpl-cli-core/src/composition.rs](../../../crates/tmpl-cli-core/src/composition.rs) |
| Noun dispatch | [crates/tmpl-cli/src/commands/mod.rs](../../../crates/tmpl-cli/src/commands/mod.rs) |
| `note` subcommands, flags, help, mapping to the app, empty-result notice | [crates/tmpl-cli/src/commands/note.rs](../../../crates/tmpl-cli/src/commands/note.rs) |
| Output mode, color, width resolution | [crates/tmpl-cli/src/output/sink.rs](../../../crates/tmpl-cli/src/output/sink.rs) |
| Rendering, error reporting, closed pipe | [crates/tmpl-cli/src/output/](../../../crates/tmpl-cli/src/output/) |
| The `note` app: `exec`, audited-command table, flows | [crates/tmpl-cli-core/src/apps/note/](../../../crates/tmpl-cli-core/src/apps/note/) |
| Next-id rule, list selection (filter, count, limit) | [crates/tmpl-cli-core/src/apps/note/rules.rs](../../../crates/tmpl-cli-core/src/apps/note/rules.rs) |
| Runtime context; audit guard around mutating commands | [crates/tmpl-cli-core/src/runtime/](../../../crates/tmpl-cli-core/src/runtime/) |
| Note and audit record shapes, validation | [crates/tmpl-cli-store/src/contract/](../../../crates/tmpl-cli-store/src/contract/) |
| The one error type and its classification | `cw-base` in [constellation-works/satellites](https://github.com/constellation-works/satellites/tree/9c9cc998720f0a75041be3451c14fbaf80f29dc8/crates/cw-base) |
| The store's codes for the kit's errors | [crates/tmpl-cli-store/src/error.rs](../../../crates/tmpl-cli-store/src/error.rs) |
| Store facade: read, write transaction | [crates/tmpl-cli-store/src/store.rs](../../../crates/tmpl-cli-store/src/store.rs) |
| The notes database: file, lock wait; schema steps per table in `schema/` | [crates/tmpl-cli-store/src/driver/sqlite/](../../../crates/tmpl-cli-store/src/driver/sqlite/) |
| Repositories: the notes' SQL and row mapping; audit | [crates/tmpl-cli-store/src/repository/](../../../crates/tmpl-cli-store/src/repository/) |
| Audit line, its upgrades | [crates/tmpl-cli-store/src/db/audit.rs](../../../crates/tmpl-cli-store/src/db/audit.rs) |
| Private SQLite database: WAL, one write transaction, read-only reader, migrations | `cw-sqlite` in [constellation-works/satellites](https://github.com/constellation-works/satellites/tree/9c9cc998720f0a75041be3451c14fbaf80f29dc8/crates/cw-sqlite) |
| Append, lock, private state | `cw-fs` in [constellation-works/satellites](https://github.com/constellation-works/satellites/tree/9c9cc998720f0a75041be3451c14fbaf80f29dc8/crates/cw-fs) |
| Goldens of help and output | [crates/tmpl-cli/tests/goldens.rs](../../../crates/tmpl-cli/tests/goldens.rs) |
