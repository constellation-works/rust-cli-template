#!/usr/bin/env bash
# check-template-version.sh — a change to a shipped file raises the template
# version.
#
# Usage: scripts/check-template-version.sh BASE
#
# Compares HEAD with its merge base with BASE (a commit or a ref such as
# origin/agent-main). When a changed path is shipped, that is, not under a
# `template` entry of scripts/template-manifest.txt, `version` in
# template.toml must be greater than BASE's (a BASE without template.toml
# counts as version 0). Exit status: 0 = fine, 1 = version not raised,
# 2 = cannot run.
set -euo pipefail
cd "$(dirname "$0")/.."

[[ $# -eq 1 ]] || { echo "usage: scripts/check-template-version.sh BASE" >&2; exit 2; }
base="$1"
version_of() { awk -F'[[:space:]]*=[[:space:]]*' '$1 == "version" { print $2; exit }'; }

git rev-parse --verify --quiet "$base^{commit}" >/dev/null \
  || { echo "template-version: cannot run: no commit $base" >&2; exit 2; }
old="$(git show "$base:template.toml" 2>/dev/null | version_of || true)"
old="${old:-0}"
new="$(version_of <template.toml)"
[[ "$old" =~ ^[0-9]+$ && "$new" =~ ^[0-9]+$ ]] \
  || { echo "template-version: cannot run: version is not an integer (base: '$old', HEAD: '$new')" >&2; exit 2; }

template_paths="$(awk '$1 == "template" { print $2 }' scripts/template-manifest.txt)"
overlay_sources="$(
  {
    awk '$1 == "overlay" { print $4 }' scripts/template-manifest.txt
    git show "$base:scripts/template-manifest.txt" 2>/dev/null | awk '$1 == "overlay" { print $4 }' || true
  } | sort -u
)"
if ! diff_output="$(git diff --name-only --no-renames "$base"...HEAD)"; then
  echo "template-version: cannot run: git diff $base...HEAD failed" >&2
  exit 2
fi
shipped=()
while IFS= read -r path; do
  [[ -n "$path" ]] || continue
  is_overlay=0
  while IFS= read -r overlay; do
    [[ -n "$overlay" ]] || continue
    if [[ "$path" == "$overlay" || "$path" == "$overlay"/* ]]; then is_overlay=1; break; fi
  done <<<"$overlay_sources"
  if [[ "$is_overlay" -eq 1 ]]; then
    shipped+=("$path")
    continue
  fi
  own=0
  while IFS= read -r prefix; do
    [[ -n "$prefix" ]] || continue
    if [[ "$path" == "$prefix" || "$path" == "$prefix"/* ]]; then own=1; break; fi
  done <<<"$template_paths"
  [[ "$own" -eq 1 ]] || shipped+=("$path")
done <<<"$diff_output"

if [[ ${#shipped[@]} -eq 0 ]]; then
  echo "template-version: ok (no shipped file changed)"
elif [[ "$new" -gt "$old" ]]; then
  echo "template-version: ok (${#shipped[@]} shipped file(s) changed; version $old -> $new)"
else
  printf '  %s\n' "${shipped[@]}" >&2
  echo "template-version: these shipped files changed; raise version in template.toml above $old" >&2
  exit 1
fi
