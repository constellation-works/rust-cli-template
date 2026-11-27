#!/usr/bin/env bash
# scripts/check-module-cycles.sh — STD-02 R6, R39: the import graph between
# units is acyclic.
#
# Crate edges cannot cycle (Cargo refuses a cyclic package graph), so this
# checks every module level in each crate's non-test source: top-level modules
# (`src/<name>.rs` or `src/<name>/`) and, recursively, the sibling modules of
# every subdirectory. An edge a -> b exists when a file of a references b via
# `crate::...`, `super::...`, or `self::...`, including grouped and nested
# `use ...::{...}` imports. Test code (`tests/` directories) is skipped. Each
# cycle is printed once as `a -> b -> a`, naming the crate and the level. Run
# by scripts/check-dependency-direction.sh; needs python3.
set -euo pipefail

cd "$(dirname "$0")/.."

command -v python3 >/dev/null 2>&1 || {
  echo "module-cycles: cannot run: python3 not found" >&2
  exit 2
}

exec python3 -I -c '
import glob, json, os, re, subprocess, sys

fails = []

def strip_comments_and_strings(text):
    """Strip Rust comments and string literals so non-code text creates no edge."""
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
        # Raw strings r#"..."# or br#"..."#
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
        out.append(ch)
        index += 1
    return "".join(out)

TOKEN = re.compile(r"::|[A-Za-z_][A-Za-z0-9_]*|[{}(),;*]")

def expand_use_trees(text):
    """Expand every `use ...;` statement into tuples of path segments."""
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

INLINE_PATH = re.compile(r"\b(?:crate|super|self)(?:::[A-Za-z_][A-Za-z0-9_]*)+")

def extract_paths(path):
    text = strip_comments_and_strings(open(path, encoding="utf-8").read())
    found = set(expand_use_trees(text))
    for match in INLINE_PATH.findall(text):
        found.add(tuple(match.split("::")))
    return found

def resolve_path(mod_path, segs):
    if not segs:
        return None
    head = segs[0]
    if head == "crate":
        return segs[1:]
    if head == "self":
        return mod_path + segs[1:]
    if head == "super":
        ups = 0
        while ups < len(segs) and segs[ups] == "super":
            ups += 1
        if ups > len(mod_path):
            return None
        return mod_path[:-ups] + segs[ups:]
    return None

def file_mod_path(src, path):
    rel = os.path.relpath(path, src)
    parts = rel.split(os.sep)
    if parts[-1] in ("lib.rs", "main.rs", "mod.rs"):
        parts = parts[:-1]
    else:
        parts[-1] = parts[-1][:-3]
    return tuple(parts)

def sources(unit):
    out = []
    if unit["file"]:
        out.append(unit["file"])
    if unit["dir"]:
        for dirpath, dirnames, filenames in os.walk(unit["dir"]):
            dirnames[:] = [dirname for dirname in sorted(dirnames) if dirname != "tests"]
            out += [os.path.join(dirpath, filename) for filename in sorted(filenames) if filename.endswith(".rs")]
    return out

def units(directory, skip):
    """The modules in a directory: name -> {file, dir}."""
    out = {}
    for entry in sorted(os.listdir(directory)):
        path = os.path.join(directory, entry)
        if entry.startswith(".") or entry in ("tests", "bin"):
            continue
        if os.path.isdir(path):
            out.setdefault(entry, {"file": None, "dir": None})["dir"] = path
        elif entry.endswith(".rs") and entry not in skip:
            out.setdefault(entry[:-3], {"file": None, "dir": None})["file"] = path
    return out

def cycles(graph):
    """Every elementary cycle once, each rotated to start at its least name."""
    found = set()
    def walk(start, node, path):
        for nxt in sorted(graph.get(node, ())):
            if nxt == start:
                found.add(tuple(path))
            elif nxt > start and nxt not in path:
                walk(start, nxt, path + [nxt])
    for start in sorted(graph):
        walk(start, start, [start])
    return sorted(found)

def report(crate, level, graph):
    for cycle in cycles(graph):
        fails.append(f"{crate}: {level}: " + " -> ".join(cycle + (cycle[0],)))

def crate_srcs():
    found = {}
    if os.path.isfile("Cargo.toml"):
        try:
            res = subprocess.run(
                ["cargo", "metadata", "--format-version", "1", "--no-deps", "--offline"],
                capture_output=True, text=True, check=True,
            )
            meta = json.loads(res.stdout)
            members = set(meta.get("workspace_members") or [])
            for pkg in meta.get("packages", []):
                if members and pkg["id"] not in members:
                    continue
                src = os.path.join(os.path.dirname(pkg["manifest_path"]), "src")
                if os.path.isdir(src):
                    found[os.path.realpath(src)] = (pkg["name"], src)
        except Exception:
            pass
    for src in sorted(glob.glob("crates/*/src")):
        real = os.path.realpath(src)
        if real not in found:
            found[real] = (os.path.basename(os.path.dirname(src)), src)
    return sorted(found.values(), key=lambda item: item[0])

crate_list = crate_srcs()
if not crate_list:
    print("module-cycles: no crate src directories; nothing to check")
    sys.exit(0)

for crate, src in crate_list:
    cache = {}
    def resolved_for(path):
        if path not in cache:
            mod_path = file_mod_path(src, path)
            out = set()
            for segs in extract_paths(path):
                resolved = resolve_path(mod_path, segs)
                if resolved:
                    out.add(resolved)
            cache[path] = out
        return cache[path]

    def check_dir(directory, prefix):
        skip = {"lib.rs", "main.rs"} if not prefix else {"mod.rs"}
        siblings = units(directory, skip)
        graph = {}
        depth = len(prefix)
        for name, unit in siblings.items():
            edges = set()
            for file in sources(unit):
                for resolved in resolved_for(file):
                    if len(resolved) > depth and resolved[:depth] == prefix:
                        target = resolved[depth]
                        if target in siblings and target != name:
                            edges.add(target)
            graph[name] = edges
        level = "top-level modules" if not prefix else "modules of " + "/".join(prefix) + "/"
        report(crate, level, graph)
        for name, unit in siblings.items():
            if unit["dir"]:
                check_dir(unit["dir"], prefix + (name,))

    check_dir(src, ())

for message in fails:
    print(f"module-cycles: {message}", file=sys.stderr)
if fails:
    print(f"module-cycles: {len(fails)} cycle(s); move what both need to the lower unit or one below both (STD-02 R39)", file=sys.stderr)
    sys.exit(1)
print("module-cycles: ok")
'
