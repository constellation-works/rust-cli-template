#!/usr/bin/env bash
# scripts/check-terminal-guard.sh — STD-01 R12/R17 and STD-02 R15.
#
# clippy::disallowed_methods in clippy.toml rejects calls to stdout, stderr
# and IsTerminal::is_terminal. This guard covers spellings that lint cannot
# resolve as calls: imports of the stream functions and terminal/format
# variable names, whose values are only visible as string literals.
set -euo pipefail
cd "$(dirname "$0")/.."

# A workspace with no crate yet has nothing to check.
if [[ ! -d crates ]]; then
  echo "terminal-guard: no crates yet; nothing to check"
  exit 0
fi

command -v python3 >/dev/null 2>&1 || {
  echo "terminal-guard: cannot run: python3 not found" >&2
  exit 2
}

OUTPUT_DIR=crates/tmpl-cli/src/output

exec python3 -I -c '
import os, re, sys

output_dir = os.path.normpath(sys.argv[1])
pattern = re.compile(
    r"\"NO_COLOR\"|\"CLICOLOR_FORCE\"|\"TERM\"|\"COLUMNS\"|TMPL_CLI_FORMAT"
)
token_re = re.compile(r"::|[A-Za-z_][A-Za-z0-9_]*|[{}(),;*]")

def strip_comments(text):
    """Replace comments with spaces, keeping newlines so line numbers stay exact."""
    out = []
    i = 0
    n = len(text)
    while i < n:
        ch = text[i]
        nxt = text[i + 1] if i + 1 < n else ""
        if ch == "/" and nxt == "/":
            i += 2
            while i < n and text[i] != "\n":
                i += 1
            continue
        if ch == "/" and nxt == "*":
            depth = 1
            i += 2
            while i < n and depth > 0:
                if i + 1 < n and text[i] == "/" and text[i + 1] == "*":
                    depth += 1
                    i += 2
                elif i + 1 < n and text[i] == "*" and text[i + 1] == "/":
                    depth -= 1
                    i += 2
                else:
                    out.append("\n" if text[i] == "\n" else " ")
                    i += 1
            continue
        out.append(ch)
        i += 1
    return "".join(out)

def imported_stdio_lines(cleaned):
    """Line numbers of `use` statements importing `io::stdout` or `io::stderr`."""
    tokens = []
    for match in token_re.finditer(cleaned):
        lineno = cleaned.count("\n", 0, match.start()) + 1
        tokens.append((match.group(0), lineno))
    n = len(tokens)
    bad_lines = set()

    def parse_tree(prefix, pos):
        segs = list(prefix)
        while pos < n:
            tok, lineno = tokens[pos]
            if tok == "{":
                pos += 1
                while pos < n and tokens[pos][0] != "}":
                    pos = parse_tree(segs, pos)
                    if pos < n and tokens[pos][0] == ",":
                        pos += 1
                if pos < n and tokens[pos][0] == "}":
                    pos += 1
                return pos
            if tok == "::":
                pos += 1
                continue
            if tok == "*":
                return pos + 1
            if tok == "as":
                pos += 1
                if pos < n and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", tokens[pos][0]):
                    pos += 1
                return pos
            if tok in (",", "}", ";"):
                return pos
            if tok == "self" and segs:
                pos += 1
                if pos < n and tokens[pos][0] == "as":
                    pos += 2
                return pos
            segs.append(tok)
            if len(segs) >= 2 and segs[-2] == "io" and segs[-1] in ("stdout", "stderr"):
                bad_lines.add(lineno)
            pos += 1
            if pos < n and tokens[pos][0] != "::":
                if tokens[pos][0] == "as":
                    pos += 1
                    if pos < n and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", tokens[pos][0]):
                        pos += 1
                return pos
        return pos

    i = 0
    while i < n:
        if tokens[i][0] == "use":
            i = parse_tree([], i + 1)
            if i < n and tokens[i][0] == ";":
                i += 1
        else:
            i += 1
    return bad_lines

hits = []
for dirpath, dirnames, filenames in os.walk("crates"):
    dirnames[:] = [d for d in sorted(dirnames) if d != "tests"]
    norm_dir = os.path.normpath(dirpath)
    if norm_dir == output_dir or norm_dir.startswith(output_dir + os.sep):
        continue
    for filename in sorted(filenames):
        if not filename.endswith(".rs"):
            continue
        path = os.path.join(dirpath, filename)
        raw_text = open(path, encoding="utf-8").read()
        raw_lines = raw_text.splitlines()
        cleaned = strip_comments(raw_text)
        cleaned_lines = cleaned.splitlines()
        matched_lines = set(imported_stdio_lines(cleaned))
        for idx, line in enumerate(cleaned_lines, 1):
            if pattern.search(line):
                matched_lines.add(idx)
        for lineno in sorted(matched_lines):
            text = raw_lines[lineno - 1] if lineno <= len(raw_lines) else ""
            hits.append(f"{path}:{lineno}:{text}")

if hits:
    for hit in hits:
        print(hit, file=sys.stderr)
    print(f"terminal-guard: terminal state and std streams belong to {output_dir} only", file=sys.stderr)
    sys.exit(1)
print("terminal-guard: ok")
' "$OUTPUT_DIR"
