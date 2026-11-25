#!/usr/bin/env python3
"""Check the frontmatter of the repository's Markdown documents.

STD-09 R15: every Markdown document at the repository root and under
`docs/` opens with YAML frontmatter carrying `type` and a one-sentence
`summary`. The root `README.md` and the root agent instruction files are
exempt, and so is the vendored `docs/standards/` (its own check verifies it).

STD-06 R4: every numbered design doc (`docs/design/<feature>/<n>_*.md`, the
templates included) carries `title`, `owner`, `created_date`,
`updated_date`, `status`, `feature`, `doc_role` and `summary`; the dates are
`YYYY-MM-DD` (the literal placeholder in `_templates/`), `updated_date` is
not before `created_date`, and `status` is `Draft`, `Accepted` or
`Deprecated`.

Prints one line per breach and exits 1 when there is one.
"""

from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent.parent
EXEMPT_ROOT = {"README.md", "AGENTS.md", "CLAUDE.md", "GEMINI.md", "CODEX.md"}
NUMBERED_FIELDS = (
    "title",
    "owner",
    "created_date",
    "updated_date",
    "status",
    "feature",
    "doc_role",
    "summary",
)
STATUSES = {"Draft", "Accepted", "Deprecated"}
DATE = re.compile(r"\d{4}-\d{2}-\d{2}")
# A sentence end followed by more text: a second sentence.
SECOND_SENTENCE = re.compile(r"[.!?][\"')\]]*\s+\S")


def documents():
    """The documents R15 covers, as paths relative to the root."""
    out = [path for path in sorted(ROOT.glob("*.md")) if path.name not in EXEMPT_ROOT]
    docs = ROOT / "docs"
    if docs.is_dir():
        out += [
            path
            for path in sorted(docs.rglob("*.md"))
            if (docs / "standards") not in path.parents
        ]
    return out


def frontmatter(path):
    """The top-level `key: value` fields, or None without frontmatter."""
    lines = path.read_text(encoding="utf-8").splitlines()
    if not lines or lines[0] != "---" or "---" not in lines[1:]:
        return None
    fields = {}
    for line in lines[1 : lines.index("---", 1)]:
        match = re.fullmatch(r"([A-Za-z_][A-Za-z0-9_]*):\s*(.*)", line)
        if match:
            key, value = match.groups()
            value = value.strip()
            if len(value) >= 2 and value[0] in "'\"" and value[0] == value[-1]:
                value = value[1:-1]
            fields[key] = value
    return fields


def check_document(rel, fields, fail):
    if fields is None:
        fail(rel, "no YAML frontmatter at the top (STD-09 R15)")
        return
    for key in ("type", "summary"):
        if not fields.get(key):
            fail(rel, f"no `{key}` in the frontmatter (STD-09 R15)")
    summary = fields.get("summary", "")
    if SECOND_SENTENCE.search(summary):
        fail(rel, "`summary` is more than one sentence (STD-09 R15)")


def check_numbered(rel, fields, template, fail):
    for key in NUMBERED_FIELDS:
        if not fields.get(key):
            fail(rel, f"no `{key}` in the frontmatter (STD-06 R4)")
    if "last_updated" in fields:
        fail(rel, "`last_updated` is not a field: use `created_date` and `updated_date` (STD-06 R4)")
    dates = {}
    for key in ("created_date", "updated_date"):
        value = fields.get(key)
        if not value:
            continue
        if template and value == "YYYY-MM-DD":
            continue
        if not DATE.fullmatch(value):
            fail(rel, f"`{key}` is not YYYY-MM-DD: {value!r} (STD-06 R4)")
        else:
            dates[key] = value
    if len(dates) == 2 and dates["updated_date"] < dates["created_date"]:
        fail(rel, "`updated_date` is before `created_date` (STD-06 R4)")
    status = fields.get("status")
    if status and status not in STATUSES:
        fail(rel, f"`status` is {status!r}, not Draft, Accepted or Deprecated (STD-06 R4)")


def main():
    breaches = []

    def fail(rel, message):
        breaches.append(f"{rel}: {message}")

    for path in documents():
        rel = path.relative_to(ROOT).as_posix()
        fields = frontmatter(path)
        check_document(rel, fields, fail)
        parts = path.relative_to(ROOT).parts
        numbered = (
            len(parts) == 4
            and parts[:2] == ("docs", "design")
            and re.match(r"\d_", parts[3]) is not None
        )
        if numbered and fields is not None:
            check_numbered(rel, fields, parts[2] == "_templates", fail)

    for line in breaches:
        print(f"doc-frontmatter: {line}", file=sys.stderr)
    if breaches:
        print(f"doc-frontmatter: {len(breaches)} breach(es)", file=sys.stderr)
        return 1
    print(f"doc-frontmatter: ok ({len(documents())} document(s))")
    return 0


if __name__ == "__main__":
    sys.exit(main())
