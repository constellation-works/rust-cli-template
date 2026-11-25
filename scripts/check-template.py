#!/usr/bin/env python3
"""The template's own gate: the note example stays removable.

Reads scripts/template-manifest.txt (its header documents the entries and the
region markers) and fails when

- a listed path is missing, or an entry is malformed;
- a marker is unbalanced, unknown or crosses another family's region;
- a file with markers is not listed `regions`, or a `regions` file has none;
- a shipped path or line names the note example outside an `example` entry,
  an example or template region, or a file the cli-bare kind replaces.

Shipped files are the repository's files (`git ls-files`) apart from the
vendored docs/standards/, symlinks, and `template` and `generated` entries.
"""

from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "scripts" / "template-manifest.txt"
FAMILIES = ("example", "crates", "template")
MARKER = re.compile(r"\b(" + "|".join(FAMILIES) + r"):(begin|end)\b")
NOTE = re.compile(r"note", re.IGNORECASE)
KINDS = ("cli", "cli-bare", "workspace")


def load_manifest(errors):
    entries = {"example": [], "crates": [], "template": [], "regions": [], "generated": [], "overlay": []}
    for number, raw in enumerate(MANIFEST.read_text(encoding="utf-8").splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        words = line.split()
        kind, args = words[0], words[1:]
        want = 3 if kind == "overlay" else 1
        if kind not in entries or len(args) != want:
            errors.append(f"{MANIFEST.name}:{number}: malformed entry: {line}")
            continue
        if kind == "overlay" and args[0] not in KINDS:
            errors.append(f"{MANIFEST.name}:{number}: unknown kind {args[0]}")
            continue
        for path in args[1:] if kind == "overlay" else args:
            if not (ROOT / path).exists():
                errors.append(f"{MANIFEST.name}:{number}: no such path: {path}")
        entries[kind].append(args if kind == "overlay" else args[0])
    return entries


def under(path, prefixes):
    return any(path == prefix or path.startswith(prefix.rstrip("/") + "/") for prefix in prefixes)


def shipped_files():
    out = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=ROOT, check=True, capture_output=True,
    ).stdout.decode("utf-8")
    for path in sorted(set(filter(None, out.split("\0")))):
        full = ROOT / path
        if path.startswith("docs/standards/") or full.is_symlink() or not full.is_file():
            continue
        yield path, full


def regions(path, lines, errors):
    """Yield (line number, text, open families) and check the markers."""
    stack = []
    for number, text in enumerate(lines, 1):
        found = MARKER.findall(text)
        if len(found) > 1:
            errors.append(f"{path}:{number}: more than one marker on a line")
        if found:
            family, edge = found[0]
            if edge == "begin":
                if family in stack:
                    errors.append(f"{path}:{number}: {family}:begin inside an open {family} region")
                stack.append(family)
            elif not stack or stack[-1] != family:
                errors.append(f"{path}:{number}: {family}:end without its begin")
            else:
                stack.pop()
            continue
        yield number, text, tuple(stack)
    if stack:
        errors.append(f"{path}: unclosed {stack[-1]} region")


def main():
    errors = []
    entries = load_manifest(errors)
    skipped = entries["template"] + entries["generated"]
    bare_overlays = [target for kind, target, _ in entries["overlay"] if kind == "cli-bare"]
    listed_regions = set(entries["regions"])
    with_markers = set()
    for path, full in shipped_files():
        if under(path, skipped):
            continue
        example_file = under(path, entries["example"])
        if NOTE.search(path) and not example_file:
            errors.append(f"{path}: names the note example; list it as `example` in {MANIFEST.name}")
        try:
            lines = full.read_text(encoding="utf-8").splitlines()
        except UnicodeDecodeError:
            continue
        if any(MARKER.search(line) for line in lines):
            with_markers.add(path)
        for number, text, open_families in regions(path, lines, errors):
            if example_file or path in bare_overlays:
                continue
            if NOTE.search(text) and not {"example", "template"} & set(open_families):
                errors.append(f"{path}:{number}: names the note example outside an example region: {text.strip()}")
    for path in sorted(with_markers - listed_regions):
        errors.append(f"{path}: has region markers; list it as `regions` in {MANIFEST.name}")
    for path in sorted(listed_regions - with_markers):
        errors.append(f"{path}: listed as `regions` but has no marker")
    if errors:
        print("\n".join(f"check-template: {error}" for error in errors), file=sys.stderr)
        return 1
    print("check-template: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
