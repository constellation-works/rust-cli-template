#!/usr/bin/env python3
"""Check every relative link and heading anchor in the repository's docs.

STD-09 R17: in each document R15 covers (the Markdown at the repository root
and under `docs/`, as `check-doc-frontmatter.py` lists them), every relative
link and image path resolves to a file or directory, and every `#anchor` on a
Markdown target names a heading or an HTML id there. A path resolves from
the linking document's directory, or from the repository root when it starts
with `/`, and must stay inside the repository: a path into a neighbouring
checkout resolves only on a machine that has one. Links with a scheme or a host (`https:`, `mailto:`) are not
fetched. The parsing is `check-readme.py`'s, which holds the README to the
same rule (R12). No network access.

Prints one line per broken link and exits 1 when there is one.
"""

from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
import sys

SCRIPTS = Path(__file__).resolve().parent
ROOT = SCRIPTS.parent


def sibling(name):
    """Load a sibling script as a module (their names are not identifiers)."""
    sys.dont_write_bytecode = True  # no __pycache__ beside the scripts
    spec = spec_from_file_location(name.replace("-", "_"), SCRIPTS / f"{name}.py")
    module = module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main():
    readme = sibling("check-readme")
    frontmatter = sibling("check-doc-frontmatter")
    anchors = {}
    broken = []
    checked = 0
    documents = frontmatter.documents()
    for path in documents:
        rel = path.relative_to(ROOT).as_posix()
        for link, target, fragment in readme.local_links(path.read_text(encoding="utf-8")):
            checked += 1
            resolved = readme.resolve_local(ROOT, path.parent, target, path)
            if resolved != ROOT and ROOT not in resolved.parents:
                broken.append(f"{rel}: link leaves the repository: {link}")
                continue
            if not resolved.exists():
                broken.append(f"{rel}: link has no target: {link}")
                continue
            if fragment and resolved.is_file() and resolved.suffix.lower() == ".md":
                if resolved not in anchors:
                    anchors[resolved] = readme.anchors(resolved)
                if fragment not in anchors[resolved]:
                    broken.append(f"{rel}: link has no heading or anchor: {link}")
    for line in broken:
        print(f"doc-links: {line}", file=sys.stderr)
    if broken:
        print(f"doc-links: {len(broken)} broken link(s) (STD-09 R17)", file=sys.stderr)
        return 1
    print(f"doc-links: ok ({checked} link(s) in {len(documents)} document(s))")
    return 0


if __name__ == "__main__":
    sys.exit(main())
