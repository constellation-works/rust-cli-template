---
title: Design Doc Conventions
owner: claude
status: Accepted
type: design
summary: How a feature's design docs under docs/design/ are laid out, what their frontmatter carries, and what earns a decision entry.
created_date: 2026-09-26
updated_date: 2026-11-25
---

# Design Doc Conventions

How design docs under `docs/design/<feature>/` are laid out, condensed from
Orbit's `docs/design/CONVENTIONS.md` (the source this template follows).

## 1. Folder layout

Copy [`1_scope.md`](_templates/1_scope.md) for a small change to an existing
mechanism. Add `4_decisions.md` when a decision meets the criteria below.

Use the full track for a subsystem that needs several design documents.

```
docs/design/<feature>/
├── 1_overview.md     what and why
├── 2_design.md       how it works today; ends with Concerns & Honest Limitations
├── 3_vision.md       open questions and directions, labelled as speculation
├── 4_decisions.md    titled decisions with Context · Decision · Consequences (Cost:)
├── specs/            one prescriptive mechanism contract per file
└── references/       glossary and other lookup docs
```

Start a feature by copying the scaffold:

```sh
mkdir -p docs/design/<feature>
cp docs/design/_templates/{1_overview,2_design,3_vision,4_decisions}.md docs/design/<feature>/
cp -r docs/design/_templates/{specs,references} docs/design/<feature>/
mv docs/design/<feature>/specs/_mechanism.md docs/design/<feature>/specs/<mechanism>.md
```

Folder names are lowercase, hyphenated and singular. No `README.md`,
roadmap, changelog or tutorial inside a feature folder.

## 2. Frontmatter and sections

Every numbered doc keeps the frontmatter from `_templates/` (`title` equals
the H1, `status` is `Draft` until reviewed, then `Accepted`; `doc_role`
matches the filename; `created_date` stays as first written and
`updated_date` changes with every content edit, STD-06 R4). The entry document also carries the delivery `stage`
and `next` step. Run `make feature-board` after changing either field. A
document that cites public tasks ends with a **Task References** section.

## 3. What earns a decision entry

A decision enters `4_decisions.md` through one of two doors:

1. **It explains surprising code** — name the file and symbol in
   `**Code anchors:**`.
2. **It governs future decisions** — a standing rule a reader can apply to a
   case nobody has seen yet.

Either way it names a real alternative and a non-trivial `Cost:`. Everything
else is design prose in `2_design.md`. A departure from an adopted
constellation standard is always a decision entry, citing `STD-nn@<version> §Rn`.

## 4. Links and ids

Link to repository files with relative paths. Cite public issues and pull
requests with a phrase describing the change. Use placeholders in examples
and help text. Private task IDs belong in the tracker.
