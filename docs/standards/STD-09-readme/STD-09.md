---
id: STD-09-readme
title: README — a short front page with a centered logo and badge row, a plain description, a quick start, and links to the docs that hold everything else
summary: Normative rules for a repository's root README.md — a front page of at most 150 lines and 1,200 words that opens with a centered logo, tagline and badge row, says what the project is before any demo, reaches a first result in a short quick start, links to the docs instead of repeating them, and states no figure that goes stale; follow it in any repository with users other than its author.
status: draft
tags: [standard, docs, readme]
version: 1
created: 2026-11-21
updated: 2026-11-21
last_validated: 2026-11-21
related: [STD-04-testing-and-verification, STD-08-economy]
---

# README

These rules cover the root `README.md` of a repository: what it holds, how long it is, and
the order it is read in. They exist so that a newcomer learns in one screen what the project
is, reaches a first result in a few minutes, and finds every other answer one link away.

Every rule is portable. The minimum is a centered header with a version and a licence badge,
a two-sentence description, a quick start, a list of doc links, and the contributing and
licence lines.

Orbit is the worked example, not the audience. Orbit paths are relative to `codebases/orbit/`.

Other standards own the neighbouring rules, and this one does not repeat them:
- **Commands and examples that run as written** are
  [STD-04](../STD-04-testing-and-verification/STD-04.md) §R14, and a README claim the code
  does not uphold is STD-04 §R13.
- **Each fact once, detail reachable by reference** is
  [STD-08](../STD-08-economy/STD-08.md). This standard applies it to the README.

The keywords MUST, MUST NOT and SHOULD are normative. A rule marked *review-only* in
[`checks.md`](checks.md) has no automated gate, so a reviewer enforces it.

This standard is a directory. This file holds the binding text: the rules, where they
apply, and how to deviate. Beside it, [`why.md`](why.md) gives the reason for each rule,
[`checks.md`](checks.md) the gate that enforces it, [`exemplars.md`](exemplars.md) worked
examples in Orbit, and [`CHANGELOG.md`](CHANGELOG.md) what changed in each version.

## Rules

### Scope and size

- **R1.** The README is the project's front page. It MUST answer four questions: what the
  project is, why someone would use it, how to install it and reach a first result, and
  where to read more. Reference material MUST live in the docs and be linked: the full
  command list, configuration keys, architecture, troubleshooting and release history.
- **R2.** The README MUST be at most 150 lines and 1,200 words, counted on the source file.
  Content over the limit moves to a doc and leaves a link.

### Header

- **R3.** The README MUST open with a centered header block, in this order: the logo, a
  one-line tagline, the badge row. The logo links to the project's site, or to the
  repository when there is no site. Its alt text is the project name. It has a dark and a
  light variant when one mark does not read on both backgrounds. A project with no logo
  uses its name as a centered `<h1>` until it has one.
- **R4.** The badge row MUST hold two to five badges, with the current release version and
  the licence among them. Each badge MUST link to the source of what it shows. A badge
  SHOULD show a value its source computes (a version, a build status), not fixed text.
- **R5.** The first prose after the header MUST say what the project is, in at most two
  sentences that a newcomer can read without the project's own vocabulary. It comes before
  any demo.
- **R6.** A demo (an animated image or a video) is optional. Where there is one, there MUST
  be only one, it MUST show the real product, it sits after the description (§R5), and its
  alt text is one sentence.

### Body

- **R7.** Body sections MUST appear in this order. Quick start, learn more, contributing
  and licence are required; the others are optional.
  1. Why, or highlights: at most five bullets.
  2. How it works: one worked example.
  3. Quick start.
  4. Features.
  5. Learn more.
  6. Contributing.
  7. Licence.

  A section may take another name. A project may add a section between quick start and
  learn more when it carries content the docs cannot (§R9).
- **R8.** Quick start MUST give the prerequisites in one line, the install as one copyable
  code block, and the path to a first visible result in at most five steps. Other install
  routes and by-hand setup go in a collapsed `<details>` block or a linked doc.
- **R9.** A section MUST carry content of its own. A heading whose body only links to docs
  MUST be folded into learn more.
- **R10.** Learn more MUST be a list of links, each with a phrase saying what the reader
  finds there. It links the documentation home and the changelog, and the configuration
  reference and the architecture or design docs where they exist.
- **R11.** Contributing and licence MUST be the last two sections, at most three lines
  each, linking the contributing guide and the licence file. Where the repository has a
  security policy, contributing links it.

### Staying true

- **R12.** Every relative link, heading anchor and image path in the README MUST resolve,
  and CI MUST check it.
- **R13.** The README MUST NOT state a figure that changes as the project runs or releases
  (a count, a benchmark, a date, a version number in prose), unless a check regenerates it.
  This includes alt text.
- **R14.** A status a user must know before adopting (pre-1.0, experimental, unsupported
  platforms) MUST be stated once, in or directly under the description.

## Applies to

`universal`: the root `README.md` of any repository with users other than its author. A
`README.md` inside a subdirectory is outside this standard.

## Deviations

An adopting repo that departs from a rule records a decision in its own design docs, citing
`STD-09@<version> §Rn` and the reason. It never edits its vendored copy of this standard.
