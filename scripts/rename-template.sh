#!/usr/bin/env bash
# rename-template.sh — turn a fresh copy of the constellation Rust CLI template
# into project NAME, then delete this script.
#
# Usage: scripts/rename-template.sh NAME
#
#   NAME  lowercase kebab-case project name (e.g. `demo-cli`); becomes the
#         binary, the crate names (`NAME`, `NAME-core`), the Rust paths
#         (`NAME_core`), the env-var prefix (`NAME_ROOT`, upper snake case) and
#         the design-doc folder.
#
# Run it once, from anywhere, in a repository created with GitHub's "Use this
# template" (or by the constellation's operations/scripts/new-rust-cli.sh,
# which calls it). It replaces the placeholder `tmpl-cli` / `tmpl_cli` /
# `TMPL_CLI` in file contents and in file and directory names, drops the
# README's template-only note, re-sorts
# Cargo.lock and re-runs rustfmt for the new name, and removes itself. It
# never touches .git/, target/ or the vendored docs/standards/, and never
# commits: review the result, run `make ci`, then commit.
set -euo pipefail

PH_KEBAB="tmpl-cli"
PH_SNAKE="tmpl_cli"
PH_UPPER="TMPL_CLI"

SELF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
ROOT="$(cd "$(dirname "$SELF")/.." && pwd)"

die() { echo "rename-template: $*" >&2; exit 1; }
usage() { sed -n '2,/^set -euo/{/^set -euo/d;s/^# \{0,1\}//;p;}' "$SELF" >&2; exit 2; }

[[ $# -eq 1 ]] || usage
NAME="$1"

[[ "$NAME" =~ ^[a-z][a-z0-9]*(-[a-z0-9]+)*$ ]] \
  || die "invalid name '$NAME': use lowercase kebab-case, e.g. demo-cli"
[[ ${#NAME} -le 40 ]] || die "name '$NAME' is longer than 40 characters"
case "$NAME" in
  core | std | alloc | test | proc-macro | build | src | target)
    die "'$NAME' collides with a Rust crate or Cargo directory name; pick another" ;;
  *tmpl-cli* | *tmpl_cli*) die "'$NAME' contains the template placeholder; pick another" ;;
esac
[[ -d "$ROOT/crates/$PH_KEBAB" ]] \
  || die "no crates/$PH_KEBAB under $ROOT; this copy was already renamed"

SNAKE="${NAME//-/_}"
UPPER="$(printf '%s' "$SNAKE" | tr '[:lower:]' '[:upper:]')"

# Delete this script first so neither pass below rewrites or renames it, and
# drop the README note that only makes sense in the template itself.
rm -f "$SELF"
perl -0pi -e 's/<!-- template-only:begin -->.*?<!-- template-only:end -->\n\n?//s' "$ROOT/README.md"

# Paths never rewritten: VCS data, build output, and the vendored standards
# (byte-identical copies checked by docs/standards/check.sh).
prune=(\( -path "$ROOT/.git" -o -path "$ROOT/target" -o -path "$ROOT/docs/standards" \) -prune)

# Contents: regular text files only. A symlink is skipped, since editing it in
# place would replace the link with a copy of its target. NAME matched
# [a-z0-9-] above, so it is safe inside the perl substitution.
find "$ROOT" "${prune[@]}" -o -type f -print0 \
  | { xargs -0 grep -lIE "$PH_KEBAB|$PH_SNAKE|$PH_UPPER" 2>/dev/null || true; } \
  | while IFS= read -r file; do
      perl -pi -e "s/$PH_UPPER/$UPPER/g; s/$PH_SNAKE/$SNAKE/g; s/$PH_KEBAB/$NAME/g" "$file"
    done

# Names: deepest first, so a directory is renamed after its contents.
find "$ROOT" -depth "${prune[@]}" -o \( -name "*$PH_KEBAB*" -o -name "*$PH_SNAKE*" \) -print \
  | while IFS= read -r path; do
      base="$(basename "$path")"
      new="${base//$PH_KEBAB/$NAME}"
      new="${new//$PH_SNAKE/$SNAKE}"
      mv "$path" "$(dirname "$path")/$new"
    done

leftover="$( (find "$ROOT" "${prune[@]}" -o -type f -print0 | xargs -0 grep -lIiE 'tmpl[-_]cli' || true
  find "$ROOT" "${prune[@]}" -o -iname '*tmpl[-_]cli*' -print) | sort -u)"
[[ -z "$leftover" ]] || die "placeholder still present in:
$leftover"

# The new name sorts differently from the placeholder, so let cargo re-sort
# Cargo.lock (no dependency version changes; offline first) and rustfmt
# re-order imports and re-wrap lines, or `make ci-fast` would fail.
if command -v cargo >/dev/null 2>&1; then
  (cd "$ROOT" && { cargo metadata --format-version 1 --offline >/dev/null 2>&1 \
    || cargo metadata --format-version 1 >/dev/null 2>&1 \
    || echo "rename-template: warning: cargo metadata failed; Cargo.lock refreshes on first build" >&2; })
  (cd "$ROOT" && cargo fmt --all) \
    || echo "rename-template: warning: cargo fmt failed; run it before make ci" >&2
else
  echo "rename-template: warning: cargo not found; run cargo fmt --all before make ci" >&2
fi

echo "Renamed the template to $NAME in $ROOT."
