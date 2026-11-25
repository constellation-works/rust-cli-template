---
title: tmpl-cli — Vision
owner: claude
last_updated: 2026-11-25
last_validated: 2026-09-26
status: Draft
feature: tmpl-cli
doc_role: vision
type: design
summary: Where a project grown from this template is expected to go next, and what the template deliberately leaves out.
tags: [tmpl-cli]
paths: ["crates/**"]
related_features: [tmpl-cli]
---

# tmpl-cli — Vision

Speculative. The note domain exists to be replaced; these are the directions
a real project usually takes next and what the template intentionally does
not build yet.

## 1. Open Questions

1. When the second noun arrives, do its verbs share one `list`/`show`
   rendering helper, or does each noun keep its own column table?
2. Does the project need NDJSON (STD-01 R7 SHOULD) once a command streams?
3. When a destructive verb appears (`note remove`), it must refuse without
   `--confirm` (STD-01 R5); is there also a bulk form that dry-runs by default?
4. If a second surface (MCP server, desktop app) appears, does the core crate
   grow an operations layer so both surfaces derive from one declaration
   (STD-01 R25)?

## 2. Prior Work

### Constellation CLIs

- Orbit's `orbit-cli` output layer (sink, payload, table, color) is the
  full-size version of what the kit's `cw-cli` now gives `output/` here.
- nebula's `neb` uses the same core-library-plus-CLI split and the same
  advisory-lock pattern.

### Conventions

- `gh` off a TTY: headerless tab-separated output, the model for the plain form.
- no-color.org for `NO_COLOR`.

## 3. What May Be Distinctive

Nothing about the note tool. The distinctive part is that every standard
rule that applies is visible in a small, working codebase with a gate behind
it.

## 4. References

**Project-internal**

- [2_design.md](./2_design.md)
- [specs/output-contract.md](./specs/output-contract.md)

**External**

- [Adopted constellation standards](../../standards/README.md).
