"""R26: no path contains `super::super`."""

import re


DEEP_SUPER = re.compile(r"\bsuper\s*::\s*(?:\{[^{}]*?\bsuper\b|super\b)")


def check_deep_super(path, lines, report):
    pieces = []
    owner = []  # the line each character of the joined code came from
    for row, line in enumerate(lines):
        code = line.code_text + " "
        pieces.append(code)
        owner.extend([row] * len(code))
    for found in DEEP_SUPER.finditer("".join(pieces)):
        report(path, owner[found.start()] + 1, "R26", "`super::super`: name the item from `crate::`")
