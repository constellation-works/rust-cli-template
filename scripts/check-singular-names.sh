#!/usr/bin/env bash
# scripts/check-singular-names.sh — STD-12 R1: a module, source directory or
# source file is named in the singular.
#
# It reads the names below each crate directory: every `.rs` file's stem and
# every directory that holds a `.rs` file. A name ending in `s` fails unless
# it is in ALLOWED. The check reads names, so it cannot tell a plural from a
# singular that ends in `s`: such a singular goes in ALLOWED with the
# toolchain-fixed names. Repository-level directories (`crates/` itself,
# `scripts/`, `docs/`) are out of R1's scope and are not read.
set -euo pipefail
cd "$(dirname "$0")/.."

# Toolchain-fixed plurals (Cargo's target directories), then singulars that
# end in `s`.
ALLOWED=(tests examples benches)

is_allowed() {
  local name=$1 allowed
  for allowed in "${ALLOWED[@]}"; do
    [[ $name == "$allowed" ]] && return 0
  done
  return 1
}

hits=()
seen=" "
while IFS= read -r -d '' path; do
  # crates/<crate>/<rest>: the crate's own name is the package's, not a module.
  rest=${path#crates/*/}
  [[ $rest == "$path" ]] && continue
  stem=$(basename "$rest" .rs)
  candidates=("$rest:$stem")
  dir=$(dirname "$rest")
  while [[ $dir != "." ]]; do
    candidates+=("${dir}/:$(basename "$dir")")
    dir=$(dirname "$dir")
  done
  for candidate in "${candidates[@]}"; do
    name=${candidate##*:}
    shown=${candidate%:*}
    [[ $name == *s ]] || continue
    is_allowed "$name" && continue
    key="${path%%/"$rest"}/$shown"
    [[ $seen == *" $key "* ]] && continue
    seen+="$key "
    hits+=("$key")
  done
done < <(git ls-files -z --cached --others --exclude-standard -- 'crates/*.rs')

if (( ${#hits[@]} > 0 )); then
  printf '%s\n' "${hits[@]}" >&2
  echo "singular-names: name each module, source directory and source file in the singular (STD-12 R1); a singular ending in s goes in ALLOWED" >&2
  exit 1
fi
echo "singular-names: ok"
