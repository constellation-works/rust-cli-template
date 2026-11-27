"""R19: a `#[rustfmt::skip]` has a comment before it."""


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
