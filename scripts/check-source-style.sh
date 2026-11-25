#!/usr/bin/env bash
# scripts/check-source-style.sh — the STD-12 source-style checks that rustfmt
# on the stable toolchain cannot make.
#
# - R12: a blank line separates the module documentation, the `use` block,
#   each top-level item and each method of an `impl` from the member before
#   and after it. Attributes, doc comments and comments belong to the item
#   they precede.
# - R13: no line of a `.rs` file is longer than rustfmt's `max_width`, apart
#   from a line that is one string literal or one URL.
# - R16: a `use` imports from one module: its brace group holds no `::`.
# - R19: a `#[rustfmt::skip]` has a comment on the line before it saying why
#   (hand-aligned or generated code).
#
# It reads every tracked or new `.rs` file outside `target/`, except a file
# that says it is generated (`@generated` in its first lines), which STD-12
# exempts. Each breach is printed as file:line: rule: message.
set -euo pipefail
cd "$(dirname "$0")/.."

max_width=$(sed -nE 's/^[[:space:]]*max_width[[:space:]]*=[[:space:]]*([0-9]+).*/\1/p' rustfmt.toml 2>/dev/null | head -n 1)
max_width=${max_width:-100}

# The file list goes in on descriptor 3: the program itself is python3's stdin.
python3 -I - "$max_width" 3< <(git ls-files -z --cached --others --exclude-standard -- '*.rs' ':!:target/**') <<'PY'
import re
import sys

MAX_WIDTH = int(sys.argv[1])
with open(3, encoding="utf-8") as listing:
    paths = [path for path in listing.read().split("\0") if path]
breaches = []


def breach(path, line_no, rule, message):
    breaches.append(f"{path}:{line_no}: {rule}: {message}")


# -- A small lexer: per line, where code is, and the brace depth -------------

class Line:
    """One source line as the lexer saw it."""

    def __init__(self, text):
        self.text = text
        self.start_depth = 0  # brace depth before the line's first character
        self.end_depth = 0
        self.code = ""  # the line with comments and literal contents blanked
        self.closes = []  # depths reached by a `;` or a closing `}` on the line
        self.in_string = False  # the line starts inside a string literal
        self.in_comment = False  # the line starts inside a block comment


def lex(text):
    """Split `text` into lines and track brace depth outside comments and literals."""
    lines = [Line(t) for t in text.split("\n")]
    depth = 0
    state = None  # None, "block", "str", "raw"
    block_level = 0
    raw_hashes = 0
    for line in lines:
        line.start_depth = depth
        line.in_string = state in ("str", "raw")
        line.in_comment = state == "block"
        src = line.text
        out = []
        pos = 0
        while pos < len(src):
            ch = src[pos]
            nxt = src[pos + 1] if pos + 1 < len(src) else ""
            if state == "block":
                if ch == "/" and nxt == "*":
                    block_level += 1
                    pos += 2
                elif ch == "*" and nxt == "/":
                    block_level -= 1
                    pos += 2
                    if block_level == 0:
                        state = None
                else:
                    pos += 1
                out.append(" ")
                continue
            if state == "str":
                if ch == "\\":
                    pos += 2
                    out.append("  ")
                    continue
                if ch == '"':
                    state = None
                    out.append('"')
                else:
                    out.append(" ")
                pos += 1
                continue
            if state == "raw":
                end = '"' + "#" * raw_hashes
                if src.startswith(end, pos):
                    state = None
                    out.append(end)
                    pos += len(end)
                else:
                    out.append(" ")
                    pos += 1
                continue
            if ch == "/" and nxt == "/":
                break
            if ch == "/" and nxt == "*":
                state = "block"
                block_level = 1
                pos += 2
                out.append("  ")
                continue
            raw = re.match(r'b?r(#*)"', src[pos:])
            if raw and (pos == 0 or not (src[pos - 1].isalnum() or src[pos - 1] == "_")):
                raw_hashes = len(raw.group(1))
                state = "raw"
                out.append(raw.group(0))
                pos += len(raw.group(0))
                continue
            if ch == '"':
                state = "str"
                out.append('"')
                pos += 1
                continue
            if ch == "'":
                char_lit = re.match(r"'(\\(x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]+\}|.)|[^\\'])'", src[pos:])
                if char_lit:
                    out.append("'" + " " * (len(char_lit.group(0)) - 2) + "'")
                    pos += len(char_lit.group(0))
                    continue
            if ch in "{([":
                depth += 1
            elif ch in "})]":
                depth -= 1
                if ch == "}":
                    line.closes.append(depth)
            elif ch == ";":
                line.closes.append(depth)
            out.append(ch)
            pos += 1
        line.code = "".join(out)
        line.end_depth = depth
    return lines


# -- R12: blank lines between items -----------------------------------------

def is_blank(line):
    return not line.in_string and not line.in_comment and line.text.strip() == ""


def is_prefix(line):
    """A doc comment, comment or outer attribute: it belongs to the next item."""
    stripped = line.text.strip()
    return stripped.startswith("//") or stripped.startswith("#[") or stripped.startswith("/*")


def first_code(lines, index):
    """The text of the first line from `index` that is not a prefix."""
    while index < len(lines) and is_prefix(lines[index]):
        index += 1
    return lines[index].text.strip() if index < len(lines) else ""


def is_use(text):
    return re.match(r"(pub(\([^)]*\))?\s+)?use\b", text) is not None


def is_fn(text):
    prefix = r"(pub(\([^)]*\))?\s+)?((const|async|unsafe|default)\s+|extern\s+(\"[^\"]*\"\s+)?)*"
    return re.match(prefix + r"fn\b", text) is not None


def is_impl(text):
    return re.match(r"(unsafe\s+)?impl\b", text) is not None


def check_blank_lines(path, lines):
    index = 0
    # The module documentation and inner attributes: one unit at the top.
    while index < len(lines) and is_blank(lines[index]):
        index += 1
    header_end = index
    while header_end < len(lines):
        stripped = lines[header_end].text.strip()
        if stripped.startswith("//!"):
            header_end += 1
        elif stripped.startswith("#!["):
            # An inner attribute may span lines: it ends back at depth 0.
            while header_end < len(lines) and lines[header_end].end_depth > 0:
                header_end += 1
            header_end += 1
        elif stripped == "" or stripped.startswith("//"):
            # A blank line or a comment inside the header, when an inner
            # attribute still follows: the attribute belongs to the module.
            ahead = header_end
            while ahead < len(lines) and (
                lines[ahead].text.strip() == "" or lines[ahead].text.strip().startswith("//")
            ):
                ahead += 1
            if ahead < len(lines) and lines[ahead].text.strip().startswith("#!["):
                header_end = ahead
            else:
                break
        else:
            break
    if header_end > index:
        if header_end < len(lines) and not is_blank(lines[header_end]):
            breach(path, header_end + 1, "R12", "no blank line after the module documentation")
        index = header_end
    check_units(path, lines, index, len(lines), 0, top_level=True)


def check_units(path, lines, begin, end, depth, top_level):
    """Check the items at `depth` in lines[begin:end]: a blank line between each."""
    index = begin
    previous = None  # (kind, last line index) of the unit before
    while index < end:
        line = lines[index]
        if is_blank(line):
            index += 1
            continue
        if line.start_depth != depth or line.in_string or line.in_comment:
            index += 1
            continue
        stripped = line.text.strip()
        if not top_level and stripped.startswith("}") and line.end_depth < depth:
            break
        unit_start = index
        head = first_code(lines, index)
        if is_use(head):
            kind = "use"
        elif not top_level and is_fn(head):
            kind = "method"
        else:
            kind = "item"
        # The unit ends on the line where a `;` or `}` brings it back to `depth`.
        unit_end = index
        while unit_end < end:
            current = lines[unit_end]
            if not is_prefix(current) or current.in_string or current.in_comment:
                if current.end_depth == depth and depth in current.closes:
                    break
                if current.end_depth < depth:
                    break
            unit_end += 1
        if previous is not None:
            gap = lines[previous[1] + 1 : unit_start]
            separated = any(is_blank(between) for between in gap)
            same_use_block = previous[0] == "use" and kind == "use"
            # In an impl, R12 separates the methods: a run of associated types
            # or constants may stay together, but a method has a blank line
            # on each side.
            grouped_members = not top_level and "method" not in (previous[0], kind)
            if not separated and not same_use_block and not grouped_members:
                where = "member of an impl" if not top_level else "top-level item"
                if kind == "use" or previous[0] == "use":
                    where = "use block"
                breach(path, unit_start + 1, "R12", f"no blank line before this {where}")
        if top_level and is_impl(head):
            body_start = next(
                (row for row in range(unit_start, min(unit_end, end - 1) + 1)
                 if lines[row].end_depth > depth),
                None,
            )
            if body_start is not None:
                check_units(path, lines, body_start + 1, unit_end + 1, depth + 1, top_level=False)
        previous = (kind, unit_end)
        index = unit_end + 1


# -- R13: line width ---------------------------------------------------------

# One string literal, maybe followed by the punctuation that closes its
# statement: a plain or byte string with escapes, or a raw string.
STRING_LINE = re.compile(r'^(b?"(?:[^"\\]|\\.)*"|b?r(#*)"(?:(?!"\2).)*"\2)[,;)]*$')
URL_LINE = re.compile(r"^(//[/!]?\s*)?<?(https?|file)://\S+>?[.,]?$")


def check_width(path, lines):
    for row, line in enumerate(lines, 1):
        if len(line.text) <= MAX_WIDTH:
            continue
        stripped = line.text.strip()
        if STRING_LINE.match(stripped):
            continue
        if URL_LINE.match(stripped):
            continue
        breach(path, row, "R13", f"{len(line.text)} characters, over max_width {MAX_WIDTH}")


# -- R16: one module per use -------------------------------------------------

def check_use_groups(path, lines):
    row = 0
    while row < len(lines):
        code = lines[row].code.strip()
        if lines[row].in_string or lines[row].in_comment or not is_use(code):
            row += 1
            continue
        start = row
        statement = code
        while ";" not in statement and row + 1 < len(lines):
            row += 1
            statement += " " + lines[row].code.strip()
        brace = statement.find("{")
        if brace != -1 and "::" in statement[brace:]:
            breach(path, start + 1, "R16", "a use brace group holds a path; split it into one use per module")
        row += 1


# -- R19: rustfmt::skip has its reason ---------------------------------------

def check_skips(path, lines):
    for row, line in enumerate(lines):
        if "rustfmt::skip" not in line.code:
            continue
        before = lines[row - 1].text.strip() if row > 0 else ""
        if not before.startswith("//") or before.startswith("///") or before.startswith("//!"):
            breach(path, row + 1, "R19", "rustfmt::skip needs a comment on the line before saying why")


for path in paths:
    with open(path, encoding="utf-8") as handle:
        text = handle.read()
    if "@generated" in "\n".join(text.split("\n")[:5]):
        continue
    if text.endswith("\n"):
        text = text[:-1]
    lines = lex(text)
    check_blank_lines(path, lines)
    check_width(path, lines)
    check_use_groups(path, lines)
    check_skips(path, lines)

if breaches:
    print("\n".join(breaches), file=sys.stderr)
    print(f"source-style: {len(breaches)} breach(es) of STD-12 (R12, R13, R16, R19)", file=sys.stderr)
    sys.exit(1)
print(f"source-style: ok ({len(paths)} files)")
PY
