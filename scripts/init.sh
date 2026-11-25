#!/usr/bin/env bash
# init.sh — turn a fresh copy of the constellation Rust CLI template into
# project NAME of one kind, then delete this script.
#
# Usage: scripts/init.sh NAME [--kind cli|cli-bare|workspace] [--repository URL]
#
#   NAME              lowercase kebab-case project name (e.g. `demo-cli`); becomes
#                     the binary, the crate names (`NAME`, `NAME-core`,
#                     `NAME-store`), the Rust paths (`NAME_core`, `NAME_store`),
#                     the env-var prefix (`NAME_ROOT`, upper snake case) and the
#                     design-doc folder.
#   --kind cli        the full template with its `note` example (the default)
#   --kind cli-bare   the layers with one read-only `status` command, no example
#   --kind workspace  an empty Cargo workspace: gates, CI, lints, agent guide
#                     and design-doc templates
#   --repository URL  the `repository` of every crate; left out without it
#
# Run it once, from anywhere, in a repository created with GitHub's "Use this
# template". scripts/template-manifest.txt says what each kind keeps: init.sh
# applies the kind's overlays, removes the files and the marked regions the
# kind does not keep, replaces the template's own text with the copy's,
# writes about.md and template.toml (the template version and the kind),
# replaces the placeholder `tmpl-cli` / `tmpl_cli` / `TMPL_CLI` in file
# contents and names, re-sorts Cargo.lock and re-runs rustfmt. It never
# touches .git/, target/ or the vendored docs/standards/, and never commits:
# review the result, run `make ci`, then commit.
set -euo pipefail

PH_KEBAB="tmpl-cli"
PH_SNAKE="tmpl_cli"
PH_UPPER="TMPL_CLI"

SELF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
ROOT="$(cd "$(dirname "$SELF")/.." && pwd)"
MANIFEST="$ROOT/scripts/template-manifest.txt"

die() { echo "init: $*" >&2; exit 1; }
usage() { sed -n '2,/^set -euo/{/^set -euo/d;s/^# \{0,1\}//;p;}' "$SELF" >&2; exit 2; }

NAME="" KIND="cli" URL=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --kind) [[ $# -ge 2 ]] || usage; KIND="$2"; shift 2 ;;
    --kind=*) KIND="${1#--kind=}"; shift ;;
    --repository) [[ $# -ge 2 ]] || usage; URL="$2"; shift 2 ;;
    --repository=*) URL="${1#--repository=}"; shift ;;
    -h | --help) usage ;;
    -*) echo "init: unknown option $1" >&2; usage ;;
    *) [[ -z "$NAME" ]] || usage; NAME="$1"; shift ;;
  esac
done
[[ -n "$NAME" ]] || usage

[[ "$NAME" =~ ^[a-z][a-z0-9]*(-[a-z0-9]+)*$ ]] \
  || die "invalid name '$NAME': use lowercase kebab-case, e.g. demo-cli"
[[ ${#NAME} -le 40 ]] || die "name '$NAME' is longer than 40 characters"
case "$NAME" in
  core | std | alloc | test | proc-macro | build | src | target)
    die "'$NAME' collides with a Rust crate or Cargo directory name; pick another" ;;
  *tmpl-cli* | *tmpl_cli*) die "'$NAME' contains the template placeholder; pick another" ;;
esac
case "$KIND" in
  cli | cli-bare | workspace) ;;
  *) die "unknown kind '$KIND': use cli, cli-bare or workspace" ;;
esac
# The URL lands in TOML and in a perl substitution: no quote, backslash,
# space or control character.
[[ -z "$URL" || "$URL" =~ ^https?://[A-Za-z0-9._~:/?#@!\&+,\;=%-]+$ ]] \
  || die "invalid repository URL '$URL': use an http(s) URL without quotes or spaces"
[[ -d "$ROOT/crates/$PH_KEBAB" && -f "$MANIFEST" ]] \
  || die "no crates/$PH_KEBAB or scripts/template-manifest.txt under $ROOT; this copy was already initialized"

VERSION="$(awk -F'[[:space:]]*=[[:space:]]*' '$1 == "version" { print $2; exit }' "$ROOT/template.toml" 2>/dev/null || true)"
[[ "$VERSION" =~ ^[0-9]+$ ]] || die "template.toml has no integer version"

# A crate named like a kit crate the workspace depends on would share its Rust
# name in the same target (an app named `cw-testkit` and the kit's dev-only
# `cw-testkit` both import as `cw_testkit`). The kit crates are the `cw-*` keys
# of the root Cargo.toml's [workspace.dependencies]; each crate this name
# produces is checked against them.
if [[ "$KIND" != workspace ]]; then
  kit_crates="$(awk '
    /^\[/ { section = $0; next }
    section == "[workspace.dependencies]" && match($0, /^cw-[a-z0-9-]+[[:space:]]*=/) {
      key = substr($0, 1, RLENGTH); sub(/[[:space:]]*=$/, "", key); print key
    }' "$ROOT/Cargo.toml" 2>/dev/null || true)"
  for crate in "$NAME" "$NAME-core" "$NAME-store"; do
    while IFS= read -r kit; do
      [[ -n "$kit" && "$crate" == "$kit" ]] \
        && die "'$NAME' would name crate $crate, which collides with the kit crate $kit the template depends on; pick another"
    done <<<"$kit_crates"
  done
fi

# --- the manifest ------------------------------------------------------------
# Read whole before anything changes: the manifest is itself removed.
example=() crates=() template=() regions=() overlays=()
while read -r entry first second third; do
  case "$entry" in
    '' | \#*) ;;
    example) example+=("$first") ;;
    crates) crates+=("$first") ;;
    template) template+=("$first") ;;
    regions) regions+=("$first") ;;
    generated) ;;
    overlay) [[ "$first" == "$KIND" ]] && overlays+=("$second $third") ;;
    *) die "unknown manifest entry '$entry'" ;;
  esac
done <"$MANIFEST"

remove=("${template[@]}") drop="template"
case "$KIND" in
  cli) ;;
  cli-bare) remove+=("${example[@]}"); drop="template,example" ;;
  workspace) remove+=("${example[@]}" "${crates[@]}"); drop="template,example,crates" ;;
esac

# --- 1. overlays, removals and regions ---------------------------------------
# The `+` form: bash 3.2 (macOS) treats an empty array as unset under `set -u`.
for overlay in ${overlays[@]+"${overlays[@]}"}; do
  read -r target source <<<"$overlay"
  mkdir -p "$(dirname "$ROOT/$target")"
  cp "$ROOT/$source" "$ROOT/$target"
done
for path in "${remove[@]}"; do
  [[ -e "$ROOT/$path" || -L "$ROOT/$path" ]] && rm -r -- "${ROOT:?}/$path"
done

# Drop the regions of the families this kind removes and the marker lines of
# the rest; a `copy:` line in a template region becomes its text. A blank
# line left doubled by a removal, and trailing blank lines, go too.
for file in "${regions[@]}"; do
  [[ -f "$ROOT/$file" ]] || continue
  DROP="$drop" VERSION="$VERSION" KIND="$KIND" perl -0777 -i -ne '
    my %drop = map { $_ => 1 } split /,/, $ENV{DROP};
    my (@stack, @out, $removed);
    for my $line (split /\n/, $_, -1) {
      if ($line =~ /\b(example|crates|template):(begin|end)\b/) {
        if ($2 eq "begin") { push @stack, $1 } else { pop @stack }
        $removed = 1;
        next;
      }
      my @dropped = grep { $drop{$_} } @stack;
      if (@dropped) {
        $removed = 1;
        next unless @dropped == 1 && $dropped[0] eq "template" && $stack[-1] eq "template";
        next unless $line =~ /^\s*(<!--|#|\/\/) copy:(.*)$/;
        my ($opener, $text) = ($1, $2);
        $text =~ s/\s*-->$// if $opener eq "<!--";
        $text =~ s/^ //;
        $line = $text;
        $line =~ s/__VERSION__/$ENV{VERSION}/g;
        $line =~ s/__KIND__/$ENV{KIND}/g;
      }
      next if $line eq "" && $removed && @out && $out[-1] eq "";
      $removed = 0 if $line ne "";
      push @out, $line;
    }
    pop @out while @out && $out[-1] eq "";
    print join("\n", @out), "\n";
  ' "$ROOT/$file"
done

# --- 2. the repository URL ---------------------------------------------------
if [[ -n "$URL" ]]; then
  export URL
  perl -pi -e 's/^(publish = false\n)/$1repository = "$ENV{URL}"\n/' "$ROOT/Cargo.toml"
  for manifest in "$ROOT"/crates/*/Cargo.toml; do
    [[ -f "$manifest" ]] || continue
    perl -pi -e 's/^(publish\.workspace = true\n)/$1repository.workspace = true\n/' "$manifest"
  done
fi

# --- 3. about.md and template.toml -------------------------------------------
# about.md describes the template. A copy gets the placeholder card every new
# codebase starts with, naming the standards its kind adopts.
if [[ "$KIND" == workspace ]]; then
  standards="STD-02,STD-03,STD-04,STD-05,STD-06,STD-08,STD-09"
else
  standards="STD-01,STD-02,STD-03,STD-04,STD-05,STD-06,STD-08,STD-09"
fi
cat >"$ROOT/about.md" <<CARD
---
codebase: $NAME
owner: daniel
type: codebase
summary: <One line — what $NAME does and for whom.>
status: active
stack: Rust
gate: pr
standards: $standards
agents: "—"
---

# $NAME — codebase card

Machine-readable layout entry for **$NAME**, read by the constellation front-door index
([\`../../operations/scripts/index.sh\`](../../operations/scripts/index.sh)) to render the
Codebases table. The human front door is [\`README.md\`](README.md) and the agent guide is
[\`AGENTS.md\`](AGENTS.md); keep \`summary\` here in sync with them.
CARD
cat >"$ROOT/template.toml" <<TOML
# The constellation Rust CLI template this project was generated from: its
# version and the kind scripts/init.sh made. A baseline for later updates.
version = $VERSION
kind = "$KIND"
TOML

# --- 4. the placeholder ------------------------------------------------------
SNAKE="${NAME//-/_}"
UPPER="$(printf '%s' "$SNAKE" | tr '[:lower:]' '[:upper:]')"
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

# Names: prune first (never with -depth, which disables -prune), then sort
# deepest first so a directory is renamed after its contents.
find "$ROOT" "${prune[@]}" -o \( -name "*$PH_KEBAB*" -o -name "*$PH_SNAKE*" \) -print0 \
  | perl -0ne 'push @p, $_; END { print for sort { ($b =~ tr|/||) <=> ($a =~ tr|/||) || length($b) <=> length($a) } @p }' \
  | while IFS= read -r -d '' path; do
      [[ "$path" == "$ROOT" ]] && continue
      base="$(basename "$path")"
      new="${base//$PH_KEBAB/$NAME}"
      new="${new//$PH_SNAKE/$SNAKE}"
      mv "$path" "$(dirname "$path")/$new"
    done

leftover="$( (find "$ROOT" "${prune[@]}" -o -type f -print0 \
    | xargs -0 grep -lIiE 'tmpl[-_]cli|(example|crates|template):(begin|end)' || true
  find "$ROOT" "${prune[@]}" -o -iname '*tmpl[-_]cli*' -print) | sort -u)"
[[ -z "$leftover" ]] || die "placeholder or region marker still present in:
$leftover"

# --- 5. generated files ------------------------------------------------------
# The kind may have removed a design folder: regenerate the feature board.
if command -v python3 >/dev/null 2>&1; then
  python3 "$ROOT/scripts/generate-feature-board.py" >/dev/null \
    || echo "init: warning: the feature board did not regenerate; run make feature-board" >&2
else
  echo "init: warning: python3 not found; run make feature-board before make ci" >&2
fi
# The new name sorts differently from the placeholder, so let cargo re-sort
# Cargo.lock (no dependency version changes; offline first) and rustfmt
# re-order imports and re-wrap lines, or `make ci-fast` would fail.
if ! compgen -G "$ROOT/crates/*/Cargo.toml" >/dev/null; then
  :
elif command -v cargo >/dev/null 2>&1; then
  (cd "$ROOT" && { cargo metadata --format-version 1 --offline >/dev/null 2>&1 \
    || cargo metadata --format-version 1 >/dev/null 2>&1 \
    || echo "init: warning: cargo metadata failed; Cargo.lock refreshes on first build" >&2; })
  (cd "$ROOT" && cargo fmt --all) \
    || echo "init: warning: cargo fmt failed; run it before make ci" >&2
else
  echo "init: warning: cargo not found; run cargo fmt --all before make ci" >&2
fi

echo "Initialized $NAME ($KIND, template version $VERSION) in $ROOT."
