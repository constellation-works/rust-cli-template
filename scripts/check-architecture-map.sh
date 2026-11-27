#!/usr/bin/env bash
# scripts/check-architecture-map.sh — STD-02 R43: ARCHITECTURE.md maps every
# unit, and the map matches the tree.
#
# Fails, naming the file and the unit, when:
#   (a) a workspace crate has no section in ARCHITECTURE.md: a `##` heading
#       whose first code span is the crate's name;
#   (b) a top-level module of a crate (`src/<name>.rs` or `src/<name>/`;
#       `tests` and the crate roots excluded) is not named in its crate's
#       section: as a top-level entry of the section's file tree, or as a
#       code span `<name>.rs` or `<name>/`;
#   (c) a path the doc writes in code formatting does not exist: a code span
#       that looks like a path (it has a `/` or a file extension), or an entry
#       of a crate section's file tree, resolved through its indentation
#       against the crate's `src/`. A span resolves from the repository root,
#       from the section's crate (`src/`, then the crate directory), or as a
#       path suffix inside that crate (inside the repository outside a crate
#       section). Placeholders (`<..>`, `$`, `*`, `…`) are skipped;
#   (d) a crate root (each lib or bin target) or a top-level module file
#       (`<name>.rs`, else `<name>/mod.rs`) does not begin with a `//!` line;
#   (e) a crate root or top-level module row with a sentence does not match
#       the unit's first `//!` line, or the unit references a sibling
#       top-level module via `crate::<name>` not named in the row's
#       `imports` clause (`imports no sibling modules` means none).
#
# The crate list comes from `cargo metadata`, so a new crate or module fails
# the gate until the doc names it, with no edit here. Needs cargo and python3.
set -euo pipefail

cd "$(dirname "$0")/.."

for tool in cargo python3; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "architecture-map: cannot run: $tool not found" >&2
    exit 2
  }
done
if ! metadata=$(cargo metadata --format-version 1 --no-deps --offline 2>/dev/null); then
  echo "architecture-map: cannot run: cargo metadata failed" >&2
  exit 2
fi

exec python3 -I -c '
import json, os, re, sys

DOC = "ARCHITECTURE.md"
root = os.getcwd()
meta = json.loads(sys.argv[1])
fails = []

def fail(msg):
    fails.append(msg)

def rel(path):
    return os.path.relpath(path, root)

# Step 1: read the crates from cargo metadata.
members = set(meta.get("workspace_members") or [])
crates = []
for pkg in meta["packages"]:
    if members and pkg["id"] not in members:
        continue
    crate_dir = os.path.dirname(pkg["manifest_path"])
    roots = [target["src_path"] for target in pkg["targets"]
             if any(kind in ("lib", "rlib", "cdylib", "staticlib", "dylib", "proc-macro", "bin")
                    for kind in target["kind"])]
    src = os.path.join(crate_dir, "src")
    crates.append({"name": pkg["name"], "dir": crate_dir, "src": src,
                   "roots": sorted(set(os.path.realpath(src_path) for src_path in roots))})
if not crates:
    print("architecture-map: workspace has no packages yet; nothing to check")
    sys.exit(0)
by_name = {crate["name"]: crate for crate in crates}

if not os.path.isfile(DOC):
    print(f"architecture-map: {DOC} is missing; it maps every crate (STD-02 R43)", file=sys.stderr)
    sys.exit(1)
lines = open(DOC, encoding="utf-8").read().split("\n")

# Step 2: read the sections, fences and code spans of the doc.
SPAN = re.compile(r"`([^`\n]+)`")
PATHISH = re.compile(r"[A-Za-z0-9_.][A-Za-z0-9_./-]*")
EXT = re.compile(r"\.(rs|sh|py|md|toml|yml|yaml|txt|json|lock)$")
PLACEHOLDER = re.compile(r"[<>$*…]")

def looks_like_path(text):
    return (PATHISH.fullmatch(text) is not None and not text.startswith("-")
            and ("/" in text or EXT.search(text) is not None))

sections = []  # (crate name or None, first line index, last line index)
current = (None, 0)
for line_index, line in enumerate(lines):
    if line.startswith("## "):
        sections.append((current[0], current[1], line_index))
        spans = SPAN.findall(line)
        current = (spans[0] if spans and spans[0] in by_name else None, line_index)
sections.append((current[0], current[1], len(lines)))

def walk_paths(base):
    """Every file and directory under base, relative to it."""
    out = set()
    for dirpath, dirnames, filenames in os.walk(base):
        dirnames[:] = [dirname for dirname in dirnames if dirname not in (".git", "target")]
        for name in dirnames + filenames:
            out.add(os.path.relpath(os.path.join(dirpath, name), base))
    return out

repo_paths = None
crate_paths = {}

def exists_span(text, crate):
    global repo_paths
    path = text.rstrip("/")
    if os.path.exists(os.path.join(root, path)):
        return True
    if crate is not None:
        if any(os.path.exists(os.path.join(base, path)) for base in (crate["src"], crate["dir"])):
            return True
        if crate["name"] not in crate_paths:
            crate_paths[crate["name"]] = walk_paths(crate["dir"])
        scope = crate_paths[crate["name"]]
    else:
        if repo_paths is None:
            repo_paths = walk_paths(root)
        scope = repo_paths
    return any(candidate == path or candidate.endswith("/" + path) for candidate in scope)

documented = {crate["name"]: set() for crate in crates}
documented_rows = {crate["name"]: [] for crate in crates}
seen_sections = set()
for crate_name, start, end in sections:
    crate = by_name.get(crate_name)
    if crate is not None:
        seen_sections.add(crate_name)
    in_fence = False
    stack = []  # (indent, directory path relative to src)
    for line_index in range(start, end):
        line = lines[line_index]
        lineno = line_index + 1
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
            stack = []
            continue
        if in_fence:
            # A crate section'"'"'s fenced block is its file tree under src/.
            if crate is None or not line.strip() or line.lstrip().startswith("<!--"):
                continue
            indent = len(line) - len(line.lstrip(" "))
            parts = re.split(r"\s{2,}", line.strip(), maxsplit=1)
            field = parts[0]
            sentence = parts[1].strip() if len(parts) > 1 else ""
            while stack and stack[-1][0] >= indent:
                stack.pop()
            parent = stack[-1][1] if stack else ""
            tokens = field.split(" ")
            if not stack and len(tokens) == 1 and sentence and not PLACEHOLDER.search(field) and looks_like_path(field):
                documented_rows[crate_name].append((lineno, field, sentence))
            for token in tokens:
                if not looks_like_path(token) and not PLACEHOLDER.search(token):
                    continue
                full = parent + token
                if not stack:
                    documented[crate_name].add(re.split(r"[/.]", token)[0])
                if token.endswith("/"):
                    stack.append((indent, full))
                if PLACEHOLDER.search(full):
                    continue
                if not os.path.exists(os.path.join(crate["src"], full)):
                    fail(f"{DOC}:{lineno}: `{crate_name}`: the tree names "
                         f"`{full}`, which does not exist under {rel(crate['"'"'src'"'"'])}/")
            continue
        for text in SPAN.findall(line):
            if crate is not None and (text.endswith(".rs") or text.endswith("/")) \
                    and PATHISH.fullmatch(text) and "/" not in text.rstrip("/"):
                documented[crate_name].add(re.split(r"[/.]", text)[0])
            if PLACEHOLDER.search(text) or not looks_like_path(text):
                continue
            if not exists_span(text, crate):
                where = f"`{crate_name}` section" if crate is not None else "the doc"
                fail(f"{DOC}:{lineno}: {where} names `{text}`, which does not exist")

# Step 3: check (a), (b), (d) and (e) for each crate.
def first_line(path):
    try:
        with open(path, encoding="utf-8") as handle:
            return handle.readline()
    except OSError:
        return ""

def needs_doc(path, unit):
    if not first_line(path).startswith("//!"):
        fail(f"{rel(path)}: `{unit}` does not begin with a `//!` doc line")

def strip_comments_and_strings(text):
    out = []
    index = 0
    length = len(text)
    while index < length:
        ch = text[index]
        nxt = text[index + 1] if index + 1 < length else ""
        if ch == "/" and nxt == "/":
            index += 2
            while index < length and text[index] != "\n":
                index += 1
            continue
        if ch == "/" and nxt == "*":
            depth = 1
            index += 2
            while index < length and depth > 0:
                if index + 1 < length and text[index] == "/" and text[index + 1] == "*":
                    depth += 1
                    index += 2
                elif index + 1 < length and text[index] == "*" and text[index + 1] == "/":
                    depth -= 1
                    index += 2
                else:
                    if text[index] == "\n":
                        out.append("\n")
                    index += 1
            out.append(" ")
            continue
        if ch in ("r", "b") and index + 1 < length:
            start = index
            scan_index = index + 1
            if ch == "b" and scan_index < length and text[scan_index] == "r":
                scan_index += 1
            if (ch == "r" or (ch == "b" and scan_index == index + 2)) and (
                start == 0 or not (text[start - 1].isalnum() or text[start - 1] == "_")
            ):
                hashes = 0
                while scan_index < length and text[scan_index] == "#":
                    hashes += 1
                    scan_index += 1
                if scan_index < length and text[scan_index] == "\"":
                    scan_index += 1
                    end_marker = "\"" + ("#" * hashes)
                    pos = text.find(end_marker, scan_index)
                    if pos == -1:
                        index = length
                    else:
                        out.append(" ")
                        index = pos + len(end_marker)
                    continue
        if ch == "\"":
            index += 1
            while index < length:
                if text[index] == "\\":
                    index += 2
                    continue
                if text[index] == "\"":
                    index += 1
                    break
                index += 1
            out.append(" ")
            continue
        if ch == chr(39) and index + 1 < length:
            # Skip char/byte literals like chr(39)+double-quote+chr(39) without consuming quotes.
            if text[index + 1] == "\\" and index + 3 < length and text[index + 3] == chr(39):
                out.append(" ")
                index += 4
                continue
            if text[index + 1] != "\\" and index + 2 < length and text[index + 2] == chr(39):
                out.append(" ")
                index += 3
                continue
        out.append(ch)
        index += 1
    return "".join(out)

TOKEN = re.compile(r"::|[A-Za-z_][A-Za-z0-9_]*|[{}(),;*]")
INLINE_PATH = re.compile(r"\bcrate(?:::[A-Za-z_][A-Za-z0-9_]*)+")

def expand_use_trees(text):
    tokens = TOKEN.findall(text)
    paths = []
    index = 0
    length = len(tokens)
    def parse_tree(prefix, pos):
        segs = list(prefix)
        while pos < length:
            tok = tokens[pos]
            if tok == "{":
                pos += 1
                while pos < length and tokens[pos] != "}":
                    pos = parse_tree(segs, pos)
                    if pos < length and tokens[pos] == ",":
                        pos += 1
                if pos < length and tokens[pos] == "}":
                    pos += 1
                return pos
            if tok == "::":
                pos += 1
                continue
            if tok == "*":
                if segs:
                    paths.append(tuple(segs))
                return pos + 1
            if tok == "as":
                pos += 1
                if pos < length and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", tokens[pos]):
                    pos += 1
                if segs:
                    paths.append(tuple(segs))
                return pos
            if tok in (",", "}", ";"):
                if segs:
                    paths.append(tuple(segs))
                return pos
            if tok == "self" and segs:
                pos += 1
                if pos < length and tokens[pos] == "as":
                    pos += 2
                paths.append(tuple(segs))
                return pos
            segs.append(tok)
            pos += 1
            if pos < length and tokens[pos] != "::":
                if tokens[pos] == "as":
                    pos += 1
                    if pos < length and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", tokens[pos]):
                        pos += 1
                paths.append(tuple(segs))
                return pos
        if segs:
            paths.append(tuple(segs))
        return pos
    while index < length:
        if tokens[index] == "use":
            index = parse_tree([], index + 1)
            if index < length and tokens[index] == ";":
                index += 1
        else:
            index += 1
    return paths

def crate_refs(path):
    try:
        text = strip_comments_and_strings(open(path, encoding="utf-8").read())
    except OSError:
        return set()
    refs = set()
    for segs in expand_use_trees(text):
        if len(segs) >= 2 and segs[0] == "crate":
            refs.add(segs[1])
    for match in INLINE_PATH.findall(text):
        segs = match.split("::")
        if len(segs) >= 2:
            refs.add(segs[1])
    return refs

def parse_imports(sentence):
    match = re.search(r"\bimports\b\s+([^.;]+)", sentence)
    if not match:
        return set()
    clause = match.group(1).strip()
    if re.fullmatch(r"no\s+sibling\s+modules?", clause) or clause in ("nothing", "none"):
        return set()
    normalized = re.sub(r"\band\b", ",", clause)
    return set(re.findall(r"[A-Za-z_][A-Za-z0-9_]*", normalized))

def walk_rs(directory):
    files = []
    if os.path.isdir(directory):
        for dirpath, dirnames, filenames in os.walk(directory):
            dirnames[:] = [d for d in sorted(dirnames) if d != "tests"]
            files.extend(os.path.join(dirpath, fn) for fn in sorted(filenames) if fn.endswith(".rs"))
    return files

def unit_files(crate, field):
    src = crate["src"]
    roots_set = set(crate["roots"])
    if field.endswith("/"):
        mod_name = field[:-1]
        mod_dir = os.path.join(src, mod_name)
        mod_rs = os.path.join(mod_dir, "mod.rs")
        alt_rs = os.path.join(src, mod_name + ".rs")
        doc_path = mod_rs if os.path.isfile(mod_rs) else (alt_rs if os.path.isfile(alt_rs) else None)
        files = ([alt_rs] if os.path.isfile(alt_rs) else []) + walk_rs(mod_dir)
        return f"{crate['"'"'name'"'"']}::{mod_name}", mod_name, doc_path, files
    file_path = os.path.join(src, field)
    if os.path.realpath(file_path) in roots_set:
        return f"{crate['"'"'name'"'"']}: crate root {field}", None, file_path, [file_path] if os.path.isfile(file_path) else []
    mod_name = field[:-3] if field.endswith(".rs") else field
    files = ([file_path] if os.path.isfile(file_path) else []) + walk_rs(os.path.join(src, mod_name))
    return f"{crate['"'"'name'"'"']}::{mod_name}", mod_name, file_path, files

for crate in crates:
    name = crate["name"]
    if name not in seen_sections:
        fail(f"{DOC}: crate `{name}` ({rel(crate['"'"'dir'"'"'])}) has no section "
             f"(a `## ` heading whose first code span is `{name}`)")
    for path in crate["roots"]:
        needs_doc(path, f"{name}: crate root {os.path.basename(path)}")
    if not os.path.isdir(crate["src"]):
        continue
    roots = set(crate["roots"])
    modules = {}
    for entry in sorted(os.listdir(crate["src"])):
        path = os.path.join(crate["src"], entry)
        if entry.startswith(".") or entry in ("tests", "bin"):
            continue
        if os.path.isdir(path):
            modules.setdefault(entry, None)
            if os.path.isfile(os.path.join(path, "mod.rs")):
                modules[entry] = modules[entry] or os.path.join(path, "mod.rs")
        elif entry.endswith(".rs") and os.path.realpath(path) not in roots:
            modules[entry[:-3]] = path
    for module, path in sorted(modules.items()):
        shown = rel(path) if path else rel(os.path.join(crate["src"], module)) + "/"
        if name in seen_sections and module not in documented[name]:
            fail(f"{DOC}: the `{name}` section does not name module `{module}` ({shown})")
        if path:
            needs_doc(path, f"{name}::{module}")
    sibling_names = set(modules.keys())
    for lineno, field, sentence in documented_rows[name]:
        unit_label, mod_name, doc_path, files = unit_files(crate, field)
        if doc_path and os.path.isfile(doc_path):
            line1 = first_line(doc_path).rstrip("\r\n")
            if line1.startswith("//!"):
                actual = line1[3:].strip()
                if actual != sentence:
                    fail(f"{rel(doc_path)}:1: `{unit_label}` first `//!` line `{actual}` does not match {DOC}:{lineno} `{sentence}`")
        allowed = parse_imports(sentence)
        siblings = sibling_names if mod_name is None else (sibling_names - {mod_name})
        referenced = set()
        for src_file in files:
            referenced |= (crate_refs(src_file) & siblings)
        for target in sorted(referenced - allowed):
            fail(f"{DOC}:{lineno}: `{unit_label}` references `crate::{target}`, which its row omits from its `imports` clause")

for message in fails:
    print(f"architecture-map: {message}", file=sys.stderr)
if fails:
    print(f"architecture-map: {len(fails)} problem(s); map each unit in {DOC} (STD-02 R43)", file=sys.stderr)
    sys.exit(1)
print(f"architecture-map: ok ({len(crates)} crate(s))")
' "$metadata"
