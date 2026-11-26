#!/usr/bin/env python3
"""Check the frontmatter of the repository's Markdown documents.

STD-09 R15: every Markdown document at the repository root and under
`docs/` opens with YAML frontmatter carrying `type` and a one-sentence
`summary`. The root `README.md` and the root agent instruction files are
exempt, and so is the vendored `docs/standards/` (its own check verifies it).

STD-09 R29: every document R15 covers carries `created_date` and
`updated_date` as `YYYY-MM-DD` (the literal placeholder in
`docs/design/_templates/`), `updated_date` not before `created_date`, and no
legacy `created`, `updated` or `last_updated` key.

STD-06 R4: every numbered design doc (`docs/design/<feature>/<n>_*.md`, the
templates included) carries `title`, `owner`, `created_date`,
`updated_date`, `status`, `feature`, `doc_role` and `summary` (the dates as
R29 above), and `status` is `Draft`, `Accepted` or `Deprecated`.

Prints one line per breach and exits 1 when there is one.
"""

from pathlib import Path
import re
import sys

try:
    import yaml  # type: ignore[import-not-found]
except ImportError:
    yaml = None

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
KEY_LINE = re.compile(r"^([A-Za-z_][A-Za-z0-9_-]*):(?:[ \t]+(.*)|[ \t]*)$")
PLAIN_FORBIDDEN_START = frozenset("&*!%@`,?{}[]|>'\"")


class FrontmatterError(ValueError):
    """Raised when a frontmatter block is not valid YAML or not a mapping."""


def _parse_with_pyyaml(block):
    class UniqueBaseLoader(yaml.BaseLoader):
        pass

    def construct_mapping(loader, node, deep=False):
        mapping = {}
        for key_node, value_node in node.value:
            key = loader.construct_object(key_node, deep=deep)
            if key in mapping:
                raise FrontmatterError(f"duplicate key {key!r}")
            mapping[key] = loader.construct_object(value_node, deep=deep)
        return mapping

    UniqueBaseLoader.add_constructor(
        yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG,
        construct_mapping,
    )
    try:
        data = yaml.load(block, Loader=UniqueBaseLoader)
    except yaml.YAMLError as err:
        detail = str(err).splitlines()[0]
        raise FrontmatterError(detail) from err
    if data is None:
        return {}
    if not isinstance(data, dict):
        raise FrontmatterError("frontmatter must be a YAML mapping")
    return data


def _strip_inline_comment(text):
    """Remove an unquoted trailing ` # ...` comment from a scalar."""
    for idx, ch in enumerate(text):
        if ch == "#" and (idx == 0 or text[idx - 1] in " \t"):
            return text[:idx].rstrip(" \t")
    return text.rstrip(" \t")


def _parse_double_quoted(text, lineno):
    out = []
    idx = 1
    length = len(text)
    escapes = {
        '"': '"',
        "\\": "\\",
        "/": "/",
        "n": "\n",
        "r": "\r",
        "t": "\t",
        "0": "\0",
        " ": " ",
    }
    while idx < length:
        ch = text[idx]
        if ch == "\\":
            idx += 1
            if idx >= length:
                raise FrontmatterError(f"line {lineno}: unterminated escape in double-quoted string")
            esc = text[idx]
            if esc in escapes:
                out.append(escapes[esc])
                idx += 1
            elif esc == "x" and idx + 2 < length:
                out.append(chr(int(text[idx + 1 : idx + 3], 16)))
                idx += 3
            elif esc == "u" and idx + 4 < length:
                out.append(chr(int(text[idx + 1 : idx + 5], 16)))
                idx += 5
            else:
                raise FrontmatterError(f"line {lineno}: invalid escape \\{esc} in double-quoted string")
        elif ch == '"':
            rest = text[idx + 1 :].lstrip(" \t")
            if rest and not rest.startswith("#"):
                raise FrontmatterError(f"line {lineno}: unexpected content after closing quote")
            return "".join(out)
        else:
            out.append(ch)
            idx += 1
    raise FrontmatterError(f"line {lineno}: unterminated double-quoted string")


def _parse_single_quoted(text, lineno):
    out = []
    idx = 1
    length = len(text)
    while idx < length:
        ch = text[idx]
        if ch == "'":
            if idx + 1 < length and text[idx + 1] == "'":
                out.append("'")
                idx += 2
            else:
                rest = text[idx + 1 :].lstrip(" \t")
                if rest and not rest.startswith("#"):
                    raise FrontmatterError(f"line {lineno}: unexpected content after closing quote")
                return "".join(out)
        else:
            out.append(ch)
            idx += 1
    raise FrontmatterError(f"line {lineno}: unterminated single-quoted string")


def _parse_flow(text, lineno):
    """Validate and parse a single-line flow sequence `[...]` or mapping `{...}`."""
    open_ch = text[0]
    close_ch = "]" if open_ch == "[" else "}"
    stack = [close_ch]
    idx = 1
    length = len(text)
    in_quote = None
    while idx < length and stack:
        ch = text[idx]
        if in_quote == '"':
            if ch == "\\":
                idx += 2
                continue
            if ch == '"':
                in_quote = None
        elif in_quote == "'":
            if ch == "'" and idx + 1 < length and text[idx + 1] == "'":
                idx += 2
                continue
            if ch == "'":
                in_quote = None
        else:
            if ch in ("'", '"'):
                in_quote = ch
            elif ch == "[":
                stack.append("]")
            elif ch == "{":
                stack.append("}")
            elif ch in ("]", "}"):
                if not stack or stack[-1] != ch:
                    raise FrontmatterError(f"line {lineno}: mismatched `{ch}` in flow collection")
                stack.pop()
            elif ch == ":" and stack[-1] == "]" and (idx + 1 == length or text[idx + 1] in " \t,[]"):
                raise FrontmatterError(f"line {lineno}: mapping values are not allowed in flow sequence")
        idx += 1
    if in_quote or stack:
        raise FrontmatterError(f"line {lineno}: unterminated flow collection")
    rest = text[idx:].lstrip(" \t")
    if rest and not rest.startswith("#"):
        raise FrontmatterError(f"line {lineno}: unexpected content after flow collection")
    return [] if open_ch == "[" else {}


def _parse_plain_scalar(text, lineno):
    value = _strip_inline_comment(text)
    if not value:
        return ""
    if value[0] in PLAIN_FORBIDDEN_START:
        raise FrontmatterError(f"line {lineno}: unquoted value starts with YAML indicator {value[0]!r}")
    if re.search(r":(?:[ \t]|$)", value):
        raise FrontmatterError(f"line {lineno}: mapping values are not allowed here (quote values containing `: `)")
    return value


def _parse_yaml_stdlib(block):
    """Strict stdlib YAML mapping parser used when PyYAML is not installed."""
    lines = block.splitlines()
    fields = {}
    idx = 0
    total = len(lines)
    while idx < total:
        raw = lines[idx]
        lineno = idx + 1
        if not raw.strip() or raw.lstrip(" ").startswith("#"):
            idx += 1
            continue
        if raw.startswith("\t"):
            raise FrontmatterError(f"line {lineno}: tab character not allowed for indentation")
        if raw.startswith(" "):
            raise FrontmatterError(f"line {lineno}: unexpected indentation")
        match = KEY_LINE.fullmatch(raw)
        if not match:
            raise FrontmatterError(f"line {lineno}: expected a YAML `key: value` mapping entry")
        key, rest = match.groups()
        if key in fields:
            raise FrontmatterError(f"line {lineno}: duplicate key {key!r}")
        rest = "" if rest is None else rest.lstrip(" \t")
        idx += 1
        if not rest or rest.startswith("#"):
            # Possibly a block sequence or nested mapping on indented lines.
            nested = []
            while idx < total:
                nxt = lines[idx]
                if not nxt.strip() or nxt.lstrip(" ").startswith("#"):
                    idx += 1
                    continue
                if nxt.startswith("\t"):
                    raise FrontmatterError(f"line {idx + 1}: tab character not allowed for indentation")
                if nxt.startswith(" "):
                    nested.append(nxt)
                    idx += 1
                else:
                    break
            if not nested:
                fields[key] = ""
            elif nested[0].lstrip(" ").startswith("-"):
                fields[key] = [line.lstrip(" ")[1:].strip() for line in nested]
            else:
                fields[key] = {}
        elif rest[0] == '"':
            fields[key] = _parse_double_quoted(rest, lineno)
        elif rest[0] == "'":
            fields[key] = _parse_single_quoted(rest, lineno)
        elif rest[0] in ("[", "{"):
            fields[key] = _parse_flow(rest, lineno)
        elif rest[0] in ("|", ">"):
            header = _strip_inline_comment(rest[1:])
            if not re.fullmatch(r"[-+]?[1-9]?|[1-9]?[-+]?", header):
                raise FrontmatterError(f"line {lineno}: invalid block scalar header {rest!r}")
            block_lines = []
            block_indent = None
            while idx < total:
                nxt = lines[idx]
                if not nxt.strip():
                    block_lines.append("")
                    idx += 1
                    continue
                if nxt.startswith("\t"):
                    raise FrontmatterError(f"line {idx + 1}: tab character not allowed for indentation")
                indent = len(nxt) - len(nxt.lstrip(" "))
                if indent == 0:
                    break
                if block_indent is None:
                    block_indent = indent
                block_lines.append(nxt[min(indent, block_indent) :])
                idx += 1
            joiner = "\n" if rest[0] == "|" else " "
            fields[key] = joiner.join(line for line in block_lines if line or rest[0] == "|").strip()
        else:
            fields[key] = _parse_plain_scalar(rest, lineno)
    return fields


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
    """The parsed YAML frontmatter mapping, or None without frontmatter.

    Raises FrontmatterError when the frontmatter block is not valid YAML or
    does not parse to a mapping.
    """
    lines = path.read_text(encoding="utf-8").splitlines()
    if not lines or lines[0] != "---" or "---" not in lines[1:]:
        return None
    block = "\n".join(lines[1 : lines.index("---", 1)])
    if yaml is not None:
        return _parse_with_pyyaml(block)
    return _parse_yaml_stdlib(block)


def check_dates(rel, fields, template, fail):
    for key in ("created", "updated", "last_updated"):
        if key in fields:
            fail(rel, f"`{key}` is a legacy key: use `created_date` and `updated_date` (STD-09 R29)")
    dates = {}
    for key in ("created_date", "updated_date"):
        value = fields.get(key)
        if not value:
            fail(rel, f"no `{key}` in the frontmatter (STD-09 R29)")
        elif not isinstance(value, str):
            fail(rel, f"`{key}` is not YYYY-MM-DD: {value!r} (STD-09 R29)")
        elif template and value == "YYYY-MM-DD":
            continue
        elif not DATE.fullmatch(value):
            fail(rel, f"`{key}` is not YYYY-MM-DD: {value!r} (STD-09 R29)")
        else:
            dates[key] = value
    if len(dates) == 2 and dates["updated_date"] < dates["created_date"]:
        fail(rel, "`updated_date` is before `created_date` (STD-09 R29)")


def check_document(rel, fields, fail):
    if fields is None:
        fail(rel, "no YAML frontmatter at the top (STD-09 R15)")
        return
    for key in ("type", "summary"):
        value = fields.get(key)
        if not isinstance(value, str) or not value.strip():
            fail(rel, f"no `{key}` in the frontmatter (STD-09 R15)")
    summary = fields.get("summary")
    if isinstance(summary, str) and SECOND_SENTENCE.search(summary):
        fail(rel, "`summary` is more than one sentence (STD-09 R15)")


def check_numbered(rel, fields, fail):
    for key in NUMBERED_FIELDS:
        if key in ("created_date", "updated_date"):
            continue  # R29's own lines name these
        value = fields.get(key)
        if not isinstance(value, str) or not value.strip():
            fail(rel, f"no `{key}` in the frontmatter (STD-06 R4)")
    status = fields.get("status")
    if isinstance(status, str) and status and status not in STATUSES:
        fail(rel, f"`status` is {status!r}, not Draft, Accepted or Deprecated (STD-06 R4)")


def main():
    breaches = []

    def fail(rel, message):
        breaches.append(f"{rel}: {message}")

    for path in documents():
        rel = path.relative_to(ROOT).as_posix()
        try:
            fields = frontmatter(path)
        except FrontmatterError as err:
            fail(rel, f"invalid YAML frontmatter: {err} (STD-09 R15)")
            continue
        check_document(rel, fields, fail)
        parts = path.relative_to(ROOT).parts
        template = parts[:3] == ("docs", "design", "_templates")
        if fields is not None:
            check_dates(rel, fields, template, fail)
        numbered = (
            len(parts) == 4
            and parts[:2] == ("docs", "design")
            and re.match(r"\d_", parts[3]) is not None
        )
        if numbered and fields is not None:
            check_numbered(rel, fields, fail)

    for line in breaches:
        print(f"doc-frontmatter: {line}", file=sys.stderr)
    if breaches:
        print(f"doc-frontmatter: {len(breaches)} breach(es)", file=sys.stderr)
        return 1
    print(f"doc-frontmatter: ok ({len(documents())} document(s))")
    return 0


if __name__ == "__main__":
    sys.exit(main())
