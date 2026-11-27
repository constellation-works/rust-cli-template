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


def check_readme_structure(text, errors):
    visible = re.sub(r"(?s)<!--.*?-->", "", without_code(text))
    _, closing_tag, body = visible.partition("</div>")
    if not closing_tag:
        errors.append("README.md header has no closing </div>")
        return

    paragraphs = [
        re.sub(r"\s+", " ", paragraph).strip()
        for paragraph in re.split(r"\n\s*\n", body)
        if paragraph.strip()
    ]
    if not paragraphs:
        errors.append("README.md needs a description after the header")
        return

    description = paragraphs[0]
    if description.startswith("#") or re.search(r"generated from|template\.toml", description, re.I):
        errors.append("README.md first prose after the header must describe the project")
    sentence_count = len(re.findall(r"[.!?](?=\s|$)", description))
    if sentence_count > 2:
        errors.append("README.md description must be at most two sentences")

    status_count = len(re.findall(r"pre-1\.0", visible, re.I))
    if status_count != 1 or not any("pre-1.0" in paragraph.lower() for paragraph in paragraphs[:2]):
        errors.append("README.md must state pre-1.0 status once in or directly under the description")

    headings = [
        (match.start(), match.group(1).strip().casefold())
        for match in re.finditer(r"^ {0,3}##[ \t]+(.+)$", body, re.M)
    ]
    required = ("quick start", "learn more", "contributing", "licence")
    positions = {}
    for title in required:
        found = next((offset for offset, heading in headings if heading == title), None)
        if found is None:
            errors.append(f"README.md needs a {title.title()} section")
        else:
            positions[title] = found

    if len(positions) == len(required):
        ordered = [positions[title] for title in required]
        if ordered != sorted(ordered):
            errors.append("README.md sections must order Quick start, Learn more, Contributing and Licence")
        quick_start = positions["quick start"]
        learn_more = positions["learn more"]
        for offset, heading in headings:
            if heading not in required and not quick_start < offset < learn_more:
                errors.append(f"README.md optional section {heading!r} must sit between Quick start and Learn more")
                break

        learn_end = min(
            (offset for offset, _ in headings if offset > learn_more),
            default=len(body),
        )
        learn_text = body[learn_more:learn_end]
        if not re.search(r"\]\((?:\./)?docs/INDEX\.md(?:#[^)]*)?\)", learn_text):
            errors.append("README.md Learn more section must link docs/INDEX.md")


def resolve_local(root, base, target, default):
    """Resolve a local link target (relative or root-relative) after symlinks."""
    if not target:
        return default.resolve()
    if target.startswith("/"):
        return (root / target.lstrip("/")).resolve()
    return (base / target).resolve()


def main():
    text = README.read_text(encoding="utf-8")
    lines, words = len(text.splitlines()), len(text.split())
    errors = []
    if lines > 150 or words > 1200:
        errors.append(f"README.md has {lines} lines and {words} words (limits: 150 and 1200)")
    check_readme_structure(text, errors)
    links = list(local_links(text))
    if not links:
        errors.append("README.md has no local links")
    for link, path, fragment in links:
        target = resolve_local(ROOT, ROOT, path, README)
        if target != ROOT and ROOT not in target.parents:
            errors.append(f"README.md link leaves the repository: {link}")
        elif not target.exists():
            errors.append(f"README.md link has no target: {link}")
        elif fragment and target.is_file() and target.suffix.lower() == ".md" and fragment not in anchors(target):
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
