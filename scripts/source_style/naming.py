"""R1: a module, source directory or source file name is singular."""

import os

from .blank_line import MOD_HEAD, strip_attributes
from .lexer import lex


ALLOWED_SINGULARS = {"status"}


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
