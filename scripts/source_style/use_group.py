"""R16: a `use` imports from one module."""

import re


USE_START = re.compile(r"(?:^|(?<=[;{}\]]))\s*(?:pub(?:\([^)]*\))?\s+)?use\s")


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
