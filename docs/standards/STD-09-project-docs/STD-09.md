---
id: STD-09-project-docs
title: Project docs — a shared structure for the README and common repository documentation
summary: Normative rules for the README and common project documentation, including shared frontmatter, an index, links, changelog, architecture, runbooks, contributing and security.
tags: [standard, docs, readme]
version: 5
created_date: 2026-11-21
updated_date: 2026-11-27
last_validated: 2026-11-21
related: [STD-04-testing-and-verification, STD-08-economy]
---

# Project docs

These rules cover the root README and common repository documentation. They give readers a short front page and a consistent, navigable set of supporting docs.

Every rule is portable. The README minimum is a centered header with a version and a licence badge, a two-sentence description, a quick start, a list of doc links, and the contributing and licence lines.

Orbit is the worked example, not the audience. Orbit paths are relative to `codebases/orbit/`.

Other standards own the neighbouring rules, and this one does not repeat them:
- **Code architecture** is [STD-02](../STD-02-rust-architecture-and-errors/STD-02.md); **feature designs** are [STD-06](../STD-06-design-docs-and-feature-tracking/STD-06.md).
- **Commands and examples that run as written** are [STD-04](../STD-04-testing-and-verification/STD-04.md) §R14, and a README claim the code does not uphold is STD-04 §R13.
- **Each fact once, detail reachable by reference** is [STD-08](../STD-08-economy/STD-08.md). This standard applies it to the README.

Lessons, RCAs and reference docs such as CONFIG and DEVELOPMENT stay outside this standard until a second repository keeps them.

The keywords MUST, MUST NOT and SHOULD are normative. A rule marked *review-only* in [`checks.md`](checks.md) has no automated gate, so a reviewer enforces it.

This standard is a directory. This file holds the binding text: the rules, where they apply, and how to deviate. Beside it, [`why.md`](why.md) gives the reason for each rule, [`checks.md`](checks.md) the gate that enforces it, [`exemplars.md`](exemplars.md) worked examples in Orbit, and [`CHANGELOG.md`](CHANGELOG.md) what changed in each version.

## Rules

### Scope and size

- **R1.** The README is the project's front page. It MUST answer four questions: what the project is, why someone would use it, how to install it and reach a first result, and where to read more. Reference material MUST live in the docs and be linked: the full command list, configuration keys, architecture, troubleshooting and release history.
- **R2.** The README MUST be at most 150 lines and 1,200 words, counted on the source file. Content over the limit moves to a doc and leaves a link.

### Header

- **R3.** The README MUST open with a centered header block, in this order: the logo, a one-line tagline, the badge row. The logo links to the project's site, or to the repository when there is no site. Its alt text is the project name. It has a dark and a light variant when one mark does not read on both backgrounds. A project with no logo uses its name as a centered `<h1>` until it has one.
- **R4.** The badge row MUST hold two to five badges, with the current release version and the licence among them. Each badge MUST link to the source of what it shows. A badge SHOULD show a value its source computes (a version, a build status), not fixed text.
- **R5.** The first prose after the header MUST say what the project is, in at most two sentences that a newcomer can read without the project's own vocabulary. It comes before any demo.
- **R6.** A demo (an animated image or a video) is optional. Where there is one, there MUST be only one, it MUST show the real product, it sits after the description (§R5), and its alt text is one sentence.

### Body

- **R7.** Body sections MUST appear in this order. Quick start, learn more, contributing and licence are required; the others are optional.
  1. Why, or highlights: at most five bullets.
  2. How it works: one worked example.
  3. Quick start.
  4. Features.
  5. Learn more.
  6. Contributing.
  7. Licence.

  A section may take another name. A project may add a section between quick start and learn more when it carries content the docs cannot (§R9).
- **R8.** Quick start MUST give the prerequisites in one line, the install as one copyable code block, and the path to a first visible result in at most five steps. Other install routes and by-hand setup go in a collapsed `<details>` block or a linked doc.
- **R9.** A section MUST carry content of its own. A heading whose body only links to docs MUST be folded into learn more.
- **R10.** Learn more MUST be a list of links, each with a phrase saying what the reader finds there. It links the documentation home and the changelog, and the configuration reference and the architecture or design docs where they exist.
- **R11.** Contributing and licence MUST be the last two sections, at most three lines each, linking the contributing guide and the licence file. Where the repository has a security policy, contributing links it.

### Staying true

- **R12.** Every relative link, heading anchor and image path in the README MUST resolve, and CI MUST check it.
- **R13.** The README MUST NOT state a figure that changes as the project runs or releases (a count, a benchmark, a date, a version number in prose), unless a check regenerates it. This includes alt text.
- **R14.** A status a user must know before adopting (pre-1.0, experimental, unsupported platforms) MUST be stated once, in or directly under the description.

### Shared docs

- **R15.** Every Markdown document at the repository root and under `docs/` MUST open with YAML frontmatter carrying `type` and `summary`, where `summary` is one sentence. The root `README.md` is exempt (§R3 requires the header block first), and so are the root agent instruction files, which STD-11 governs.
- **R16.** `docs/INDEX.md` MUST be generated by a repository script from §R15 summaries except feature design folders; it MUST link `docs/design/FEATURES.md` once instead. CI MUST check the generated output.
- **R17.** Every relative link and heading anchor in a document §R15 covers MUST resolve, and CI MUST check it.
- **R27.** In the constellation canon under `docs/standards/`, each Markdown paragraph, list item and table row MUST occupy one source line; prose MUST NOT be hard-wrapped at a column. Line breaks within a block are permitted where Markdown gives them meaning, including fenced code blocks, deliberate hard breaks, nested list items and YAML front matter; table rows remain separate lines.
- **R29.** Every project document that carries YAML frontmatter MUST carry `created_date` and `updated_date`, both formatted as `YYYY-MM-DD`. `created_date` MUST record the document's creation date and MUST NOT change on subsequent edits; `updated_date` MUST change with every content edit, and legacy `created`, `updated` and `last_updated` keys MUST NOT be used.
- **R30.** Where a document records when its commands, procedures or claims were last checked against the live system or code, that date MUST be kept in `last_validated` (`YYYY-MM-DD`), separate from `updated_date`, so prose edits do not claim re-validation.

### Changelog

- **R18.** A repository that ships releases MUST keep a `CHANGELOG.md` at its root that opens with an `## Unreleased` section, then one `## <version> - <YYYY-MM-DD>` section per release, newest first.
- **R19.** Breaking changes MUST sit under their own `### Breaking` heading in their release section, each as one line with a migration in the form `X -> do Y`.
- **R20.** `CHANGELOG.md` MUST be compiled at release time from merged work. A task or change MUST NOT edit it.

### Architecture

- **R21.** The root `ARCHITECTURE.md` MUST hold the layering rules and the component dependency table. A repository has one architecture map: no second file holds it.
- **R22.** A script in the repository MUST enforce every edge in the component dependency table. A change that alters an edge MUST update the table in the same commit.
- **R23.** With more than one component, `docs/architecture/README.md` MUST give the end-to-end data flow and link one page per component. Each page `docs/architecture/<component>.md` MUST have the sections What it owns, Main modules, Dependencies and Entry points; it MUST describe the code as it is and link `docs/design` for intent.

### Runbooks

- **R24.** A runbook MUST cover one operator goal or one failure mode, MUST lead with the command, and MUST NOT carry a host inventory or a credential. A runbook directory has no README; the index lists the runbooks.
- **R28.** A runbook file (excluding templates such as `_template.md`) MUST be named `RB-<nn>-<slug>.md`, where `nn` is a two-digit sequence number that is never reused and `slug` is lowercase kebab-case. Its frontmatter `id` MUST equal the file name without `.md`.

### Contributing and security

- **R25.** A repository MUST have a `SECURITY.md` that gives private reporting, scope and known limits.
- **R26.** A repository MUST have a `CONTRIBUTING.md` that gives setup, the required checks, and how agents may be used.

## Applies to

`universal`: the root `README.md` and Markdown documents at the repository root and under `docs/` of any repository with users other than its author, excluding root agent instruction files, which STD-11 governs. Individual rules state their own applicability and exemptions.

## Deviations

An adopting repo that departs from a rule records a decision in its own design docs, citing `STD-09@<version> §Rn` and the reason. It never edits its vendored copy of this standard.
