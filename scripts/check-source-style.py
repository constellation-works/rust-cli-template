#!/usr/bin/env python3
"""The STD-12 source-style checks that stable rustfmt cannot make.

Run through scripts/check-source-style.sh (R12, R13, R16, R19) and
scripts/check-singular-names.sh (R1); each passes its mode as the first
argument. Every tracked or new `.rs` file is read, apart from a file whose
leading comment block says `@generated`, which STD-12 exempts. Each breach is
printed as file:line: rule: message, and any breach exits 1.

- R1 (`names`): a module, source directory or source file name ending in `s`
  fails unless it is allowed. `examples` and `benches` are allowed only as a
  crate-root directory, where Cargo fixes them. `tests` is allowed as the
  crate-root directory and as a module's unit-test directory and module,
  which STD-02 R19 fixes (`<module>/tests/`, `mod tests;`). Module names are
  read from `mod` declarations, inline or with `#[path]`, as well as from
  paths. ALLOWED_SINGULARS holds singulars that end in `s`.
- R12: a blank line separates the module documentation (`//!`, `/*! */` and
  inner attributes) from what follows, the use block from the items around
  it, each top-level item from the next, and each method of an `impl`
  (file-scope or in an inline module) from the member before and after it.
  Doc comments, comments and attributes belong to the item they precede, and
  a comment inside a use block does not split it.
- R13: no line is longer than 100 characters, the width R11 and R13 fix;
  `rustfmt.toml` must not set `max_width` to anything else. A line that lies
  within one string literal (plain, byte, C or raw, maybe followed by the
  punctuation that closes its statement) or is one URL (`scheme://…`, bare,
  in `<…>` or after a comment marker) is exempt.
- R16: a `use` imports from one module: its brace group holds no `::`. A `use`
  is found after an attribute on the same line or after a block comment.
- R19: a `#[rustfmt::skip]` has a comment (`//` or `/* */`, not a doc comment)
  on the line before it.

Known limits, left to review:
- R1 reads names, so it cannot tell an irregular plural (`children`) from a
  singular, nor a plural from a singular ending in `s` outside the allow-list.
- R16 also reads a `use` written inside a macro's tokens (`stringify!`) as an
  import.
- R19 checks that the comment is there, not that it gives a real reason.
- R19 finds `rustfmt::skip` as written; a spelling with a comment inside the
  path (`#[rustfmt /* … */ :: skip]`) is not found.
"""

import os
import re
import subprocess
import sys

WIDTH = 100
ALLOWED_SINGULARS = {"status"}
GENERATED = "@generated"

URL = re.compile(r"^(//[/!]?|/\*[*!]?|\*)?\s*<?[A-Za-z][A-Za-z0-9+.-]*://\S+?>?[.,;:)]*(\s*\*/)?$")
USE_START = re.compile(r"(?:^|(?<=[;{}\]]))\s*(?:pub(?:\([^)]*\))?\s+)?use\s")
FN_HEAD = re.compile(
    r"^(pub(\([^)]*\))?\s+)?((const|async|unsafe|default)\s+|extern\s+(\"[^\"]*\"\s+)?)*fn\b"
)
IMPL_HEAD = re.compile(r"^((unsafe|default)\s+)*impl\b")
MOD_HEAD = re.compile(r"^(pub(\([^)]*\))?\s+)?mod\s+(r#)?([A-Za-z_][A-Za-z0-9_]*)\s*([;{])")
USE_HEAD = re.compile(r"^(pub(\([^)]*\))?\s+)?use\b")


class Line:
    """One source line as the lexer saw it."""

    def __init__(self, text, start):
        self.text = text
        self.start = start  # offset of the line's first character in the file
        self.start_depth = 0  # brace depth before the line's first character
        self.end_depth = 0
        self.code = []  # the line with comments and literal contents blanked
        self.closes = []  # depths a `;` or a closing `}` on the line returns to
        self.comments = []  # (kind, text) of each comment piece on the line
        self.starts_in_comment = False

    @property
    def has_code(self):
        return "".join(self.code).strip() != ""

    @property
    def code_text(self):
        return "".join(self.code)

    def is_blank(self):
        return self.text.strip() == "" and not self.starts_in_comment


def lex(text):
    """Lines with brace depth, code, comments, and the literal tokens' spans."""
    lines = []
    offset = 0
    for raw in text.split("\n"):
        lines.append(Line(raw, offset))
        offset += len(raw) + 1
    line_of = []
    for index, line in enumerate(lines):
        line_of.extend([index] * (len(line.text) + 1))
    literals = []  # (start, end) offsets of every string literal token
    depth = 0
    pos = 0
    length = len(text)
    current = 0

    def emit(char, at):
        lines[line_of[at]].code.append(char)

    def advance_to(new_pos):
        nonlocal current
        while current + 1 < len(lines) and lines[current + 1].start <= new_pos:
            current += 1
            lines[current].start_depth = depth
            lines[current].end_depth = depth

    while pos < length:
        advance_to(pos)
        char = text[pos]
        nxt = text[pos + 1] if pos + 1 < length else ""
        if char == "\n":
            pos += 1
            continue
        if char == "/" and nxt == "/":
            end = text.find("\n", pos)
            end = length if end == -1 else end
            body = text[pos:end]
            if body.startswith("//!"):
                kind = "inner_doc"
            elif body.startswith("///") and not body.startswith("////"):
                kind = "outer_doc"
            else:
                kind = "line"
            lines[line_of[pos]].comments.append((kind, body))
            for at in range(pos, end):
                emit(" ", at)
            pos = end
            continue
        if char == "/" and nxt == "*":
            if text.startswith("/*!", pos):
                kind = "inner_doc"
            elif text.startswith("/**", pos) and not text.startswith("/**/", pos):
                kind = "outer_doc"
            else:
                kind = "block"
            level = 0
            scan = pos
            while scan < length:
                if text.startswith("/*", scan):
                    level += 1
                    scan += 2
                elif text.startswith("*/", scan):
                    level -= 1
                    scan += 2
                    if level == 0:
                        break
                else:
                    scan += 1
            for at in range(pos, scan):
                if text[at] != "\n":
                    emit(" ", at)
            first, last = line_of[pos], line_of[min(scan, length) - 1]
            for row in range(first, last + 1):
                lo = max(pos, lines[row].start)
                hi = min(scan, lines[row].start + len(lines[row].text))
                lines[row].comments.append((kind, text[lo:hi]))
                if row > first:
                    lines[row].starts_in_comment = True
            pos = scan
            continue
        boundary = pos == 0 or not (text[pos - 1].isalnum() or text[pos - 1] == "_")
        raw = re.match(r'(?:b|c)?r(#*)"', text[pos:]) if boundary else None
        if raw:
            hashes = raw.group(1)
            close = text.find('"' + hashes, pos + len(raw.group(0)))
            end = length if close == -1 else close + 1 + len(hashes)
            literals.append((pos, end))
            for at in range(pos, end):
                if text[at] != "\n":
                    emit('"' if at in (pos, end - 1) else " ", at)
            pos = end
            continue
        plain = re.match(r'(?:b|c)?"', text[pos:]) if boundary or char == '"' else None
        if plain:
            scan = pos + len(plain.group(0))
            while scan < length and text[scan] != '"':
                scan += 2 if text[scan] == "\\" else 1
            end = min(scan + 1, length)
            literals.append((pos, end))
            for at in range(pos, end):
                if text[at] != "\n":
                    emit('"' if at in (pos, end - 1) else " ", at)
            pos = end
            continue
        if char == "'" or (char == "b" and nxt == "'" and boundary):
            start = pos + (1 if char == "b" else 0)
            char_lit = re.match(r"'(\\(x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]+\}|.)|[^\\'\n])'", text[start:])
            if char_lit:
                end = start + len(char_lit.group(0))
                for at in range(pos, end):
                    emit(" ", at)
                pos = end
                continue
        if char in "{([":
            depth += 1
        elif char in "})]":
            depth -= 1
            if char == "}":
                lines[line_of[pos]].closes.append(depth)
        elif char == ";":
            lines[line_of[pos]].closes.append(depth)
        emit(char, pos)
        lines[line_of[pos]].end_depth = depth
        pos += 1
    advance_to(length)
    return lines, literals


def leading_comments_say_generated(lines):
    """True when a comment in the file's leading comment block says @generated."""
    for line in lines:
        if line.has_code:
            return False
        if any(GENERATED in body for _, body in line.comments):
            return True
    return False


# -- R12 ---------------------------------------------------------------------

def strip_attributes(code):
    """The code after any outer attributes that open the line."""
    rest = code.lstrip()
    while rest.startswith("#[") or rest.startswith("#!["):
        level = 0
        for at, char in enumerate(rest):
            if char == "[":
                level += 1
            elif char == "]":
                level -= 1
                if level == 0:
                    rest = rest[at + 1 :].lstrip()
                    break
        else:
            return ""
    return rest


def is_comment_only(line):
    return not line.has_code and not line.is_blank()


def attribute_end(lines, row):
    """The last line of an attribute that opens at `row` and spans lines."""
    depth = lines[row].start_depth
    while row < len(lines) - 1 and lines[row].end_depth > depth:
        row += 1
    return row


def module_header_end(lines):
    """The index past the module documentation and inner attributes, or 0."""
    row = 0
    last = -1
    while row < len(lines):
        line = lines[row]
        code = line.code_text.strip()
        if line.is_blank() or is_comment_only(line):
            if any(kind == "inner_doc" for kind, _ in line.comments):
                last = row
            row += 1
            continue
        if code.startswith("#!["):
            row = attribute_end(lines, row)
            last = row
            row += 1
            continue
        break
    return last + 1


def check_blank_lines(path, lines, report):
    begin = module_header_end(lines)
    if begin and begin < len(lines) and not lines[begin].is_blank():
        report(path, begin + 1, "R12", "no blank line after the module documentation")
    scan_units(path, lines, begin, len(lines), 0, "module", report)


def scan_units(path, lines, begin, end, depth, mode, report):
    """The items at `depth` in lines[begin:end], in `mode` module, impl or nested."""
    row = begin
    previous = None
    while row < end:
        line = lines[row]
        if line.is_blank():
            row += 1
            continue
        if line.start_depth != depth:
            row += 1
            continue
        if line.has_code and line.code_text.strip().startswith("}") and line.end_depth < depth:
            break
        unit_start = row
        head_row = None
        cursor = row
        while cursor < end:
            current = lines[cursor]
            if current.is_blank() and head_row is None:
                break
            if is_comment_only(current):
                cursor += 1
                continue
            stripped = strip_attributes(current.code_text)
            if stripped == "" and current.code_text.strip().startswith("#"):
                cursor = attribute_end(lines, cursor) + 1
                continue
            head_row = cursor
            break
        if head_row is None:
            row = cursor if cursor > row else row + 1
            continue
        head = strip_attributes(lines[head_row].code_text)
        unit_end = head_row
        while unit_end < end:
            current = lines[unit_end]
            if current.end_depth < depth:
                break
            if current.end_depth == depth and depth in current.closes:
                break
            unit_end += 1
        unit_end = min(unit_end, end - 1)
        if USE_HEAD.match(head):
            kind = "use"
        elif mode == "impl" and FN_HEAD.match(head):
            kind = "method"
        else:
            kind = "item"
        if previous is not None and mode != "nested":
            gap = lines[previous[1] + 1 : unit_start]
            separated = any(between.is_blank() for between in gap)
            same_use_block = previous[0] == "use" and kind == "use"
            grouped_members = mode == "impl" and "method" not in (previous[0], kind)
            if not separated and not same_use_block and not grouped_members:
                if mode == "impl":
                    where = "member of an impl"
                elif "use" in (kind, previous[0]):
                    where = "use block"
                else:
                    where = "top-level item"
                report(path, unit_start + 1, "R12", f"no blank line before this {where}")
        if mode != "impl":
            body = body_range(lines, head_row, unit_end, depth)
            if body is not None and IMPL_HEAD.match(head):
                scan_units(path, lines, body[0], body[1], depth + 1, "impl", report)
            elif body is not None and MOD_HEAD.match(head):
                scan_units(path, lines, body[0], body[1], depth + 1, "nested", report)
        previous = (kind, unit_end)
        row = unit_end + 1


def body_range(lines, head_row, unit_end, depth):
    """The rows inside the item's braces, when its body spans lines."""
    for row in range(head_row, unit_end + 1):
        if lines[row].end_depth == depth + 1 and "{" in lines[row].code_text:
            return (row + 1, unit_end + 1) if row < unit_end else None
    return None


# -- R13 ---------------------------------------------------------------------

def within_one_literal(line, literals):
    """True when the line's text lies inside one string literal token."""
    text = line.text
    first = len(text) - len(text.lstrip())
    last = len(text.rstrip())
    tail = text[:last].rstrip(",;)")
    lo, hi = line.start + first, line.start + len(tail)
    if hi <= lo:
        return False
    return any(start <= lo and hi <= end for start, end in literals)


def check_width(path, lines, literals, report):
    for row, line in enumerate(lines, 1):
        if len(line.text) <= WIDTH:
            continue
        if within_one_literal(line, literals):
            continue
        if URL.match(line.text.strip()):
            continue
        report(path, row, "R13", f"{len(line.text)} characters, over {WIDTH}")


# -- R16 ---------------------------------------------------------------------

def check_use_groups(path, lines, report):
    for row, line in enumerate(lines):
        code = line.code_text
        for found in USE_START.finditer(code):
            statement = code[found.end() :]
            cursor = row
            while ";" not in statement and cursor + 1 < len(lines):
                cursor += 1
                statement += " " + lines[cursor].code_text
            statement = statement.split(";", 1)[0]
            brace = statement.find("{")
            if brace != -1 and "::" in statement[brace:]:
                report(path, row + 1, "R16", "a use brace group holds a path; one use per module")


# -- R19 ---------------------------------------------------------------------

def check_skips(path, lines, report):
    for row, line in enumerate(lines):
        if "rustfmt::skip" not in line.code_text:
            continue
        before = lines[row - 1] if row > 0 else None
        reason = before is not None and not before.has_code and any(
            kind in ("line", "block") for kind, _ in before.comments
        )
        if not reason:
            report(path, row + 1, "R19", "rustfmt::skip needs a comment on the line before saying why")


# -- R1 ----------------------------------------------------------------------

def crate_root(path):
    """The directory of the nearest Cargo.toml above `path`, or None."""
    folder = os.path.dirname(path)
    while folder:
        if os.path.exists(os.path.join(folder, "Cargo.toml")):
            return folder
        folder = os.path.dirname(folder)
    return "." if os.path.exists("Cargo.toml") else None


def plural(name):
    return name.endswith("s") and name not in ALLOWED_SINGULARS


def check_names(paths, report):
    seen = set()
    for path in paths:
        root = crate_root(path)
        if root is None:
            continue
        rel = os.path.relpath(path, root)
        parts = rel.split(os.sep)
        for index, part in enumerate(parts):
            is_file = index == len(parts) - 1
            name = part[:-3] if is_file and part.endswith(".rs") else part
            shown = os.path.join(root, *parts[: index + 1]) + ("" if is_file else "/")
            if shown in seen or not plural(name):
                continue
            seen.add(shown)
            if index == 0 and not is_file and name in ("tests", "examples", "benches"):
                continue
            if index > 0 and not is_file and name == "tests" and parts[0] == "src":
                continue
            report(shown, 0, "R1", "name it in the singular")
        with open(path, encoding="utf-8") as handle:
            lines, _ = lex(handle.read())
        for row, line in enumerate(lines):
            head = strip_attributes(line.code_text)
            declared = MOD_HEAD.match(head)
            if declared and plural(declared.group(4)) and declared.group(4) != "tests":
                report(path, row + 1, "R1", f"module `{declared.group(4)}`: name it in the singular")


# -- Entry -------------------------------------------------------------------

def tracked_sources():
    listing = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", "*.rs"],
        check=True,
        capture_output=True,
    ).stdout.decode()
    return sorted(
        path for path in set(listing.split("\0"))
        if path and not path.startswith("target/") and os.path.exists(path)
    )


def configured_width():
    """The max_width rustfmt.toml sets, or None when it sets none."""
    if not os.path.exists("rustfmt.toml"):
        return None
    with open("rustfmt.toml", encoding="utf-8") as handle:
        for raw in handle:
            found = re.match(r"\s*max_width\s*=\s*(\S+)", raw)
            if found:
                return found.group(1)
    return None


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "style"
    breaches = []

    def report(path, row, rule, message):
        where = f"{path}:{row}" if row else path
        breaches.append(f"{where}: {rule}: {message}")

    sources = []
    for path in tracked_sources():
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
        lines, literals = lex(text[:-1] if text.endswith("\n") else text)
        if leading_comments_say_generated(lines):
            continue
        sources.append((path, lines, literals))

    if mode == "names":
        check_names([path for path, _, _ in sources], report)
        label = "singular-names"
        rules = "R1"
    else:
        width = configured_width()
        if width is not None and width != str(WIDTH):
            report("rustfmt.toml", 0, "R13", f"max_width is {width}; STD-12 R11 fixes it at {WIDTH}")
        for path, lines, literals in sources:
            check_blank_lines(path, lines, report)
            check_width(path, lines, literals, report)
            check_use_groups(path, lines, report)
            check_skips(path, lines, report)
        label = "source-style"
        rules = "R12, R13, R16, R19"

    if breaches:
        print("\n".join(breaches), file=sys.stderr)
        print(f"{label}: {len(breaches)} breach(es) of STD-12 ({rules})", file=sys.stderr)
        return 1
    print(f"{label}: ok ({len(sources)} files)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
