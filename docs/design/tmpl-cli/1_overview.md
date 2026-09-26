---
title: tmpl-cli — Overview
owner: claude
last_updated: 2026-09-26
last_validated: 2026-09-26
status: Draft
feature: tmpl-cli
doc_role: overview
type: design
summary: A small note-keeping CLI whose real job is to be the working example of the constellation CLI, Rust-architecture, process-safety, testing and security standards.
tags: [tmpl-cli, cli, standards]
paths: ["crates/**"]
related_features: [tmpl-cli]
related_artifacts: [ORB-13117, ORB-13134]
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
- **Store** — `notes.json` plus `.lock` in an owner-only data directory;
  replaced atomically, written under an advisory lock, format-versioned.
- **Payload** — the typed record every output mode renders from; its JSON
  form is the machine contract.
- **Sink** — the per-invocation answer to "who reads stdout": output mode,
  color, width. Resolved once, in one module.

## 3. At a Glance

| Concern | File | Task |
|---------|------|------|
| Command tree, flags, help, examples | [crates/tmpl-cli/src/cli.rs](../../../crates/tmpl-cli/src/cli.rs) | [ORB-13117] |
| Data directory and store construction | [crates/tmpl-cli/src/app.rs](../../../crates/tmpl-cli/src/app.rs) | [ORB-13117] |
| Verb dispatch, empty-result notice | [crates/tmpl-cli/src/command.rs](../../../crates/tmpl-cli/src/command.rs) | [ORB-13117] |
| Output mode, color, width resolution | [crates/tmpl-cli/src/output/sink.rs](../../../crates/tmpl-cli/src/output/sink.rs) | [ORB-13117] |
| Rendering, error reporting, closed pipe | [crates/tmpl-cli/src/output/](../../../crates/tmpl-cli/src/output/) | [ORB-13117] |
| Note types and validation | [crates/tmpl-cli-core/src/note.rs](../../../crates/tmpl-cli-core/src/note.rs) | [ORB-13117] |
| Store, atomic write, lock, private state, format upgrades | [crates/tmpl-cli-core/src/store.rs](../../../crates/tmpl-cli-core/src/store.rs), [fsio.rs](../../../crates/tmpl-cli-core/src/fsio.rs) | [ORB-13117], [ORB-13134] |
| Goldens of help and output | [crates/tmpl-cli/tests/goldens.rs](../../../crates/tmpl-cli/tests/goldens.rs) | [ORB-13117] |

## Task References

- [ORB-13117] — built the Rust CLI template this project was created from.
- [ORB-13134] — brought the template to STD-01..03 v2 and STD-04/05 v1.

> Resolve any task above with `orbit task show <ID>` or `git log --grep=<ID>`.
