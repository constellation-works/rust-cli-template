# Why — STD-09 README

Why each rule in [STD-09](STD-09.md) exists, tied to what was observed where there was
something. Read the entry for a rule when its intent is unclear or before deviating. The
rules themselves are in `STD-09.md`; this file explains them and adds none.

The standard starts from the owner's words in the chief's chat on 2026-11-21: "I like
orbit's README.md - succinct, short, references links to important docs, badges present with
centered logo. demo video is optional." Orbit's README at `e83ed04050` is the base. The
READMEs measured that day: Orbit 121 lines and 1,038 words, nebula 133 and 984, scribe 394
and 3,135, constellation-site 206, orbit-decision-mail 194.

- **R1 (front page).** scribe's README grew to 3,135 words by holding the manuals for its
  Codex plugin, its Claude Code package and its tmux popup (lines 157–328). A README that
  tries to be the manual is read by nobody to the end.
- **R2 (size).** "Succinct, short" needs a number or it erodes one paragraph at a time.
  The limits sit about a fifth above Orbit's README, which the owner named as the right
  size, and nebula's fits too.
- **R3 (header).** The owner asked for a centered logo. The order is Orbit's. A linked logo
  gives the reader the project's home in the first element.
- **R4 (badges).** The owner asked for badges. Version and licence are the two facts a
  reader checks before installing. More than five badges wrap into a second row and stop
  being read. Orbit's docs badge is fixed text that repeats the logo's link, which is why
  computed values are preferred.
- **R5 (description first).** In Orbit's README the tagline is a slogan ("Agents write.
  Orbit delivers.") and the sentence saying what Orbit is sits under an 880-pixel demo.
  A reader on a registry page or a phone sees a logo and an animation before learning what
  the project is.
- **R6 (demo).** The owner: "demo video is optional." One demo keeps the page short. Orbit's
  demo alt text is about 160 words, which a screen reader reads in full and nobody edits.
- **R7 (order).** The order is Orbit's, with its link-only sections removed (§R9). A fixed
  order lets a reader who knows one constellation project find the quick start in another.
- **R8 (quick start).** Orbit's quick start reaches a merged pull request in two commands
  and one paragraph, and keeps the by-hand route in a collapsed block. Its one paragraph
  holds five steps in prose, which is the most a reader follows without numbering.
- **R9 (no link-only sections).** Orbit's README has three headings (crews and complexity
  routing, MCP and authority, where state lives) whose bodies are one or two sentences of
  links, and a table row that links to one of them, which then links onward.
- **R10 (learn more).** "References links to important docs" is the owner's third point. A
  bare link list does not tell the reader which doc answers the question in hand.
- **R11 (closing sections).** Contributors and licence checkers look at the end of the page.
  Orbit has a `SECURITY.md` that its README does not link.
- **R12 (links resolve).** A README links more files than any other doc, so a renamed doc
  or heading breaks the front page silently. Orbit's `scripts/check-doc-links.py` checks
  Markdown links and heading anchors in CI. It does not read HTML attributes, so the logo
  and demo sources in the centered header are unchecked.
- **R13 (no ageing figures).** Orbit's demo alt text states that 6,894 tasks were done in
  30 days, 47 auto-task definitions and 11,190 tasks, as of the day it was recorded. Each
  figure is wrong a day later and none is regenerated.
- **R14 (status once).** Orbit says it is pre-1.0 only in the changelog bullet of its last
  list, where a reader deciding whether to adopt will not look.
