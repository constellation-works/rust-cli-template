#!/usr/bin/env bash
# scripts/check-module-cycles.sh — STD-02 R6, R39: the import graph between
# units is acyclic.
#
# Crate edges cannot cycle (Cargo refuses a cyclic package graph), so this
# checks the two module levels the template has, in each crate's non-test
# source:
#   (a) its top-level modules (`src/<name>.rs` or `src/<name>/`): an edge
#       a -> b when a file of a names `crate::b`;
#   (b) the modules of each top-level directory (one layer, such as the
#       core's `app/`): an edge a -> b when a file of a names
#       `crate::<dir>::b`, or `super::b` from a module directly in it.
# Test code (a `tests/` directory) is not a unit and is skipped. Each cycle
# is printed once as `a -> b -> a`, naming the crate and the level. Run by
# scripts/check-dependency-direction.sh; needs python3.
set -euo pipefail

cd "$(dirname "$0")/.."

if ! compgen -G 'crates/*/src' >/dev/null; then
  echo "module-cycles: no crates/*/src yet; nothing to check"
  exit 0
fi

command -v python3 >/dev/null 2>&1 || {
  echo "module-cycles: cannot run: python3 not found" >&2
  exit 2
}

exec python3 -I -c '
import glob, os, re, sys

fails = []

def sources(path):
    """The non-test .rs files of a unit: the file itself, or its directory."""
    if os.path.isfile(path):
        return [path]
    out = []
    for dirpath, dirnames, filenames in os.walk(path):
        dirnames[:] = [d for d in dirnames if d != "tests"]
        out += [os.path.join(dirpath, f) for f in filenames if f.endswith(".rs")]
    return out

def code(path):
    """The file without comments, so a doc line naming a path is no edge."""
    text = open(path, encoding="utf-8").read()
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return re.sub(r"//[^\n]*", "", text)

def units(directory, skip):
    """The modules in a directory: name -> path (file or folder)."""
    out = {}
    for entry in sorted(os.listdir(directory)):
        path = os.path.join(directory, entry)
        if entry.startswith(".") or entry in ("tests", "bin"):
            continue
        if os.path.isdir(path):
            out[entry] = path
        elif entry.endswith(".rs") and entry not in skip:
            out[entry[:-3]] = path
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

for src in sorted(glob.glob("crates/*/src")):
    crate = os.path.basename(os.path.dirname(src))
    top = units(src, {"lib.rs", "main.rs"})
    graph = {}
    for name, path in top.items():
        edges = set()
        for file in sources(path):
            for target in re.findall(r"\bcrate::([a-z_][a-z0-9_]*)", code(file)):
                if target in top and target != name:
                    edges.add(target)
        graph[name] = edges
    report(crate, "top-level modules", graph)
    for layer, path in top.items():
        if not os.path.isdir(path):
            continue
        inner = units(path, {"mod.rs"})
        graph = {}
        for name, sub in inner.items():
            edges = set()
            direct = sub if os.path.isfile(sub) else os.path.join(sub, "mod.rs")
            for file in sources(sub):
                text = code(file)
                pattern = r"\bcrate::" + layer + r"::([a-z_][a-z0-9_]*)"
                targets = re.findall(pattern, text)
                if file == direct:
                    targets += re.findall(r"\bsuper::([a-z_][a-z0-9_]*)", text)
                edges |= {t for t in targets if t in inner and t != name}
            graph[name] = edges
        report(crate, f"modules of {layer}/", graph)

for message in fails:
    print(f"module-cycles: {message}", file=sys.stderr)
if fails:
    print(f"module-cycles: {len(fails)} cycle(s); move what both need to the lower unit or one below both (STD-02 R39)", file=sys.stderr)
    sys.exit(1)
print("module-cycles: ok")
'
