---
title: Kit — Scope
owner: claude
created_date: 2026-11-27
updated_date: 2026-11-27
status: Accepted
feature: kit
doc_role: scope
type: design
summary: The decisions every kind of copy keeps, a record of each standard deviation and each rule left unexercised by the code and gate scripts that copy still carries.
stage: shipped
next: —
---

# Kit — Scope

## Problem

`scripts/init.sh` removes the design folder of the example with the example.
A deviation recorded only there left every other kind of copy, because the
code and the gate scripts that deviate stay: a project made with `cli-bare` or
`workspace` carried the deviation and no record of it, and a reviewer reading
its decisions could not tell a deliberate departure from a mistake.

## Goal

Every kind of copy holds, in [4_decisions.md](./4_decisions.md), an active
decision for each standard deviation that its own code and scripts carry, and
for each rule they leave unexercised. Nothing in the folder names the example,
so it survives in every kind.

## Shape

- A decision about a gate script, a Makefile target or a vendored file sits
  outside any region and reaches every kind.
- A decision about the crates sits inside a crates region of the file, so the
  `workspace` kind, which has no crate, drops it.
- `scripts/template-manifest.txt` lists the file as `regions`.
- A deviation the example alone carries stays in the example's own decisions.

## Verification

`scripts/check-template.py` names each decision by title and the kinds that
carry it, and fails when a title is missing, superseded, or sits on the wrong
side of a `crates` region. `crates/tmpl-cli/tests/template.rs` runs the same
check on the copy `scripts/init.sh` makes for each kind, and the template's CI
runs it on each kind's scratch copy.
