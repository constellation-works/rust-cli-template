"""R12: blank lines between the module documentation, use blocks, items and impl methods."""

import re


FN_HEAD = re.compile(
    r"^(pub(\([^)]*\))?\s+)?((const|async|unsafe|default)\s+|extern\s+(\"[^\"]*\"\s+)?)*fn\b"
)
IMPL_HEAD = re.compile(r"^((unsafe|default)\s+)*impl\b")
MOD_HEAD = re.compile(r"^(pub(\([^)]*\))?\s+)?mod\s+(r#)?([A-Za-z_][A-Za-z0-9_]*)\s*([;{])")
USE_HEAD = re.compile(r"^(pub(\([^)]*\))?\s+)?use\b")


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
