#!/usr/bin/env python3
"""Check the README size and local link targets without network access."""

from html.parser import HTMLParser
from pathlib import Path
import re
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parent.parent
README = ROOT / "README.md"


class HtmlLinks(HTMLParser):
    def __init__(self):
        super().__init__()
        self.links = []
        self.anchors = set()

    def handle_starttag(self, tag, attrs):
        for key, value in attrs:
            if not value:
                continue
            if key in {"href", "src"}:
                self.links.append(value)
            elif key == "srcset":
                self.links.extend(item.strip().split()[0] for item in value.split(",") if item.strip())
            elif key == "id" or (tag == "a" and key == "name"):
                self.anchors.add(value)


def without_code(text):
    return re.sub(r"(?ms)^ {0,3}(```+|~~~+)[^\n]*\n.*?^ {0,3}\1[ \t]*$", "", text)


def anchors(path):
    text = without_code(path.read_text(encoding="utf-8"))
    html = HtmlLinks()
    html.feed(text)
    found = set()
    for line in re.findall(r"^ {0,3}#{1,6}[ \t]+(.+)$", text, re.M):
        heading = re.sub(r"\[([^\]]+)\]\([^)]+\)|<[^>]+>", r"\1", line)
        slug = re.sub(r"[^\w -]", "", heading.strip().lower()).replace(" ", "-")
        candidate = slug
        suffix = 1
        while candidate in found:
            candidate = f"{slug}-{suffix}"
            suffix += 1
        found.add(candidate)
    return found | html.anchors


def local_links(text):
    text = without_code(text)
    html = HtmlLinks()
    html.feed(text)
    inline = re.findall(r"\]\(\s*(<[^>]+>|[^\s)]+)", text)
    references = re.findall(r"^ {0,3}\[[^\]]+\]:\s*(<[^>]+>|\S+)", text, re.M)
    for link in sorted(set(inline + references + html.links)):
        target = urlsplit(link.strip("<>"))
        if not target.scheme and not target.netloc:
            yield link, unquote(target.path), unquote(target.fragment)


def main():
    text = README.read_text(encoding="utf-8")
    lines, words = len(text.splitlines()), len(text.split())
    errors = []
    if lines > 150 or words > 1200:
        errors.append(f"README.md has {lines} lines and {words} words (limits: 150 and 1200)")
    links = list(local_links(text))
    if not links:
        errors.append("README.md has no local links")
    for link, path, fragment in links:
        target = ROOT / path if path else README
        if not target.exists():
            errors.append(f"README.md link has no target: {link}")
        elif fragment and target.suffix.lower() == ".md" and fragment not in anchors(target):
            errors.append(f"README.md link has no heading or anchor: {link}")

    manifest = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    package = manifest.split("[workspace.package]", 1)[1].split("\n[", 1)[0]
    version = re.search(r'^version\s*=\s*"([^"]+)"', package, re.M)
    if not version or f"/badge/version-{version[1].replace('-', '--')}-" not in text:
        errors.append("README.md version badge must match the Cargo.toml workspace version")
    if errors:
        raise SystemExit("\n".join(errors))
    print(f"README.md: {lines} lines, {words} words, {len(links)} local links checked")


if __name__ == "__main__":
    main()
