# Machine checks — STD-09 project docs

The gate that enforces each rule in [STD-09](STD-09.md), or `review-only`. Ship a change with the gate its rule names, or say it is review-only.

| Rule | Gate |
|---|---|
| R1 | review-only. |
| R2 | A CI step that fails when `wc -l README.md` exceeds 150 or `wc -w README.md` exceeds 1,200. Orbit: none yet. |
| R3 | review-only. |
| R4 | review-only. |
| R5 | review-only. |
| R6 | review-only. |
| R7 | review-only. |
| R8 | review-only; the commands themselves are covered by STD-04 §R14. |
| R9 | review-only. |
| R10 | review-only. |
| R11 | review-only. |
| R12 | A CI link check over `README.md` that resolves relative paths, heading anchors and image sources, including those in HTML attributes. Orbit: `scripts/check-doc-links.py` in `scripts/ci-guardrails.sh` covers Markdown links and anchors, not HTML `src`, `srcset` or `href`. |
| R13 | review-only. |
| R14 | review-only. |
| R15 | review-only |
| R16 | Review the generated index for the feature board link and excluded feature design folders; CI checks regeneration. |
| R17 | CI: link and anchor check over root and docs/ Markdown |
| R18 | review-only |
| R19 | review-only |
| R20 | review-only |
| R21 | review-only |
| R22 | CI: the edge script |
| R23 | review-only |
| R24 | review-only |
| R25 | review-only |
| R26 | review-only |
| R27 | Canon lint (`line_wrapped`) in `.orbit-plugin/lib/standards/canon.py`, run by `standards.status` and `standards.amend`. |
| R28 | review-only. Optionally, a CI check over `docs/runbooks/` verifies each non-template file matches `^RB-[0-9]{2}-[a-z0-9-]+\.md$` and its frontmatter `id` matches the filename stem. |
| R29 | review-only. Optionally, a CI frontmatter lint checks that every frontmatter block carries `created_date` and `updated_date` matching `^[0-9]{4}-[0-9]{2}-[0-9]{2}$` and rejects `created`, `updated` and `last_updated`. |
| R30 | review-only. A frontmatter lint MAY validate `last_validated` as `YYYY-MM-DD` when present. |
