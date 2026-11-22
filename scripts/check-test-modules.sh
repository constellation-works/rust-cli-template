#!/usr/bin/env bash
# scripts/check-test-modules.sh — STD-02 R19: every unit-test directory under
# `src/**/tests/` is declared (`mod tests;`) by its parent module, and every
# unit-test file inside it is declared in that directory's `mod.rs`.
# An undeclared module or file compiles to nothing, so its tests silently never run.
set -euo pipefail
cd "$(dirname "$0")/.."

fail=0

# 1. Every `src/**/tests/` directory must be declared by its parent module.
while IFS= read -r test_dir; do
  parent_dir=$(dirname "$test_dir")
  candidates=()
  if [[ "$(basename "$parent_dir")" == "src" ]]; then
    [[ -f "$parent_dir/lib.rs" ]] && candidates+=("$parent_dir/lib.rs")
    [[ -f "$parent_dir/main.rs" ]] && candidates+=("$parent_dir/main.rs")
  else
    [[ -f "$parent_dir/mod.rs" ]] && candidates+=("$parent_dir/mod.rs")
    [[ -f "${parent_dir}.rs" ]] && candidates+=("${parent_dir}.rs")
  fi

  declared=0
  for parent in "${candidates[@]}"; do
    if grep -qE '^[[:space:]]*(pub(\([^)]*\))?[[:space:]]+)?mod[[:space:]]+tests[[:space:]]*;' "$parent"; then
      declared=1
      break
    fi
  done

  if [[ "$declared" -eq 0 ]]; then
    echo "test-modules: $test_dir is not declared by its parent module (add \`mod tests;\`)" >&2
    fail=1
  fi
done < <(find crates -path '*/src/*' -type d -name tests | sort)

# 2. Every `.rs` file inside a `tests/` directory must be declared in `tests/mod.rs`.
while IFS= read -r file; do
  dir=$(dirname "$file")
  name=$(basename "$file" .rs)
  [[ "$name" == "mod" ]] && continue
  if [[ ! -f "$dir/mod.rs" ]] || ! grep -qE "^[[:space:]]*mod[[:space:]]+${name}[[:space:]]*;" "$dir/mod.rs"; then
    echo "test-modules: $file is not declared in $dir/mod.rs (add \`mod $name;\`)" >&2
    fail=1
  fi
done < <(find crates -path '*/src/*' -path '*/tests/*' -name '*.rs' | sort)

if [[ "$fail" -eq 0 ]]; then
  echo "test-modules: ok"
fi
exit "$fail"
