"""R13: no line is longer than the width STD-12 fixes."""

import re


WIDTH = 100
URL = re.compile(r"^(//[/!]?|/\*[*!]?|\*)?\s*<?[A-Za-z][A-Za-z0-9+.-]*://\S+?>?[.,;:)]*(\s*\*/)?$")


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
