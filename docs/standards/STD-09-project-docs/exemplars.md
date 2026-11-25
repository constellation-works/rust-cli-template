# Exemplars — STD-09 project docs

Worked examples of the rules in [STD-09](STD-09.md), as pointers into the Orbit codebase. Orbit is the example, not the audience: this file is optional reading, and useful only with an Orbit checkout.

Paths are relative to `codebases/orbit/`. Line numbers are at `e83ed04050`.

- R2, followed — `README.md`: 121 lines, 1,038 words.
- R3, followed — `README.md` lines 1–10: the linked lockup with dark and light sources, then the tagline.
- R4, followed — `README.md` lines 12–17: release, npm, licence and docs badges, each linked. The docs badge is fixed text.
- R5, counter-example — `README.md` line 23: the description sits under the demo (line 20).
- R6, R13, counter-example — `README.md` line 20: the demo's alt text is one paragraph of about 160 words that states dated counts.
- R8, followed — `README.md` `## Quick start`: one prerequisites line, a two-command install block, the by-hand route inside `<details>`.
- R9, counter-example — `README.md` `## Crews and complexity routing`, `## MCP and authority`, `## Where state lives`: headings whose bodies only link to docs.
- R10, followed — `README.md` `## Learn more`: each link says what the doc holds.
- R11, followed — `README.md` `## Contributing` and `## License`; `SECURITY.md` is not linked.
- R12, partly followed — `scripts/check-doc-links.py`: Markdown links and heading anchors, run from `scripts/ci-guardrails.sh`; HTML attributes are not read.
- R14, counter-example — `README.md` `## Learn more`, the changelog bullet: the only place the pre-1.0 status is stated.
