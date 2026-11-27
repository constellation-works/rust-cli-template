"""The Rust lexer every source-style check reads: lines with brace depth, code and comments."""

import re


GENERATED = "@generated"


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
