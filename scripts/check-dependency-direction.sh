#!/usr/bin/env bash
# scripts/check-dependency-direction.sh — STD-02 R1–R6, R16, R17 for this workspace.
#
# Two layers of checks, both from source files alone (no build, no network,
# no jq), so the gate runs in `make ci-fast`:
#
#   1. Crate edges. Every workspace crate needs a policy below. A crate may
#      depend only on the internal crates in its allowlist, and never on the
#      external crates in its banlist. Dev-dependencies are exempt. A crate
#      without a policy fails the check, so a new member forces a decision.
#   2. Module edges inside a crate, as grep and grouped-import bans over
#      non-test sources.
#
# The layer order is written down in ARCHITECTURE.md; change it and this
# script in the same commit (STD-02 R6).
set -euo pipefail
cd "$(dirname "$0")/.."

PREFIX="tmpl-cli"
fail=0
err() { echo "dependency-direction: $*" >&2; fail=1; }

# --- 1. crate policies -------------------------------------------------------
# policy <crate>  -> sets ALLOWED (internal deps) and BANNED (external deps)
policy() {
  case "$1" in
    tmpl-cli-core)
      ALLOWED=""
      # The domain never parses arguments, draws on a terminal or installs a
      # log subscriber: those belong to surfaces.
      BANNED="clap tracing-subscriber anstream anstyle termcolor terminal_size crossterm owo-colors colored"
      ;;
    tmpl-cli)
      ALLOWED="tmpl-cli-core"
      BANNED=""
      ;;
    *) return 1 ;;
  esac
}

# Print the dependency names declared in the non-dev dependency tables of a
# Cargo.toml: [dependencies], [build-dependencies] and target-specific ones.
declared_deps() {
  awk '
    /^\[/ {
      section = $0
      in_deps = (section ~ /^\[(target\..*\.)?(build-)?dependencies\]$/)
      next
    }
    in_deps && /^[A-Za-z0-9_-]+[[:space:]]*(\.workspace)?[[:space:]]*=/ {
      name = $1; sub(/\.workspace$/, "", name); sub(/=.*/, "", name)
      gsub(/[[:space:]]/, "", name); print name
    }
  ' "$1"
}

contains() { # contains <word> <space-separated list>
  local word="$1" item
  for item in $2; do [[ "$item" == "$word" ]] && return 0; done
  return 1
}

for manifest in crates/*/Cargo.toml; do
  crate=$(awk -F'"' '/^name[[:space:]]*=/ { print $2; exit }' "$manifest")
  if ! policy "$crate"; then
    err "crate '$crate' ($manifest) has no policy; add one here and a row in ARCHITECTURE.md"
    continue
  fi
  while IFS= read -r dep; do
    if [[ "$dep" == "$PREFIX" || "$dep" == "$PREFIX"-* ]]; then
      contains "$dep" "$ALLOWED" || err "$crate must not depend on internal crate $dep"
    elif contains "$dep" "$BANNED"; then
      err "$crate must not depend on $dep (surface crate in the domain)"
    fi
  done < <(declared_deps "$manifest")
done

# --- 2. module bans ----------------------------------------------------------
# ban <path> <extended-regex> <message>: fail if the pattern appears in
# non-test, non-comment lines of the Rust sources under <path> (a file or a
# directory).
ban() {
  local hits
  hits=$(grep -rnHE --include='*.rs' --exclude-dir=tests "$2" "$1" 2>/dev/null \
    | grep -vE '^[^:]+:[0-9]+:[[:space:]]*//' || true)
  if [[ -n "$hits" ]]; then
    echo "$hits" >&2
    err "$3"
  fi
}

# ban_import <path> <prefix> <members_regex> <message>: fail if a grouped
# `use <prefix>::{ ... }` (single- or multi-line) imports any direct member
# matching <members_regex> in non-test, non-comment Rust sources.
ban_import() {
  local target="$1" prefix="$2" members="$3" msg="$4"
  local files=() hits
  while IFS= read -r f; do
    [[ -n "$f" ]] && files+=("$f")
  done < <(find "$target" -name '*.rs' -not -path '*/tests/*' 2>/dev/null | sort)
  [[ ${#files[@]} -eq 0 ]] && return 0
  hits=$(awk -v prefix="$prefix" -v members="$members" '
    FNR == 1 { in_use = 0; depth = 0; skip_next = 0; skip_rest = 0 }
    {
      line = $0
      sub(/\/\/.*/, "", line)
      if (!in_use) {
        if (line ~ "^[[:space:]]*(pub(\\([^)]*\\))?[[:space:]]+)?use[[:space:]]+(::)?" prefix "::\\{") {
          in_use = 1
          depth = 0
          skip_next = 0
          skip_rest = 0
          sub("^[[:space:]]*(pub(\\([^)]*\\))?[[:space:]]+)?use[[:space:]]+(::)?" prefix "::", "", line)
        } else {
          next
        }
      }
      token = ""
      n = length(line)
      for (i = 1; i <= n; i++) {
        c = substr(line, i, 1)
        if (c == "{") {
          if (depth == 1 && token != "" && !skip_next && !skip_rest && token ~ "^(" members ")$") {
            printf "%s:%d:%s\n", FILENAME, FNR, $0
          }
          token = ""
          depth++
        } else if (c == "}") {
          if (depth == 1 && token != "" && !skip_next && !skip_rest && token ~ "^(" members ")$") {
            printf "%s:%d:%s\n", FILENAME, FNR, $0
          }
          token = ""
          depth--
          if (depth <= 0) {
            in_use = 0
            break
          }
        } else if (depth == 1 && !skip_rest && c ~ /[A-Za-z0-9_]/) {
          token = token c
        } else {
          if (depth == 1 && token != "") {
            if (!skip_next && !skip_rest && token ~ "^(" members ")$") {
              printf "%s:%d:%s\n", FILENAME, FNR, $0
            }
            if (token == "as") {
              skip_next = 1
            } else {
              skip_next = 0
            }
            token = ""
          }
          if (depth == 1 && c == ":") {
            skip_rest = 1
          } else if (depth == 1 && c == ",") {
            skip_rest = 0
            skip_next = 0
          }
        }
      }
      if (in_use && depth == 1 && token != "" && !skip_next && !skip_rest && token ~ "^(" members ")$") {
        printf "%s:%d:%s\n", FILENAME, FNR, $0
      }
    }
  ' "${files[@]}" || true)
  if [[ -n "$hits" ]]; then
    echo "$hits" >&2
    err "$msg"
  fi
}

CORE=crates/tmpl-cli-core/src
CLI=crates/tmpl-cli/src

# Core is business logic plus one persistence module (R5). Every core module
# but store/ is I/O-free and knows nothing of how notes are stored; a new
# business module is covered as soon as it exists.
for path in "$CORE"/*; do
  name=$(basename "$path")
  case "$name" in store | tests) continue ;; esac
  ban "$path" 'std::(fs|net|process)|tempfile::|rustix::' "core/$name is business logic: file and process access belong in store/"
  ban_import "$path" 'std' 'fs|net|process'               "core/$name is business logic: file and process access belong in store/"
  ban "$path" 'crate::store'                              "core/$name is business logic: it must not depend on persistence"
  ban_import "$path" 'crate' 'store'                      "core/$name is business logic: it must not depend on persistence"
done
# Inside store/: fsio is mechanism with no knowledge of notes, and the format
# and audit record shapes do no I/O of their own (R4, R5).
ban "$CORE/store/fsio.rs"   'crate::(note|query|store)|super::'  "store/fsio.rs is mechanism only; it must not know notes or the store"
ban_import "$CORE/store/fsio.rs" 'crate' 'note|query|store'      "store/fsio.rs is mechanism only; it must not know notes or the store"
ban "$CORE/store/format.rs" 'std::fs|super::fsio'                "store/format.rs converts bytes; it must not do I/O"
ban_import "$CORE/store/format.rs" 'std' 'fs'                    "store/format.rs converts bytes; it must not do I/O"
ban_import "$CORE/store/format.rs" 'super' 'fsio'                "store/format.rs converts bytes; it must not do I/O"
ban "$CORE/store/audit.rs"  'std::fs|super::fsio'                "store/audit.rs is a record shape; it must not do I/O"
ban_import "$CORE/store/audit.rs" 'std' 'fs'                     "store/audit.rs is a record shape; it must not do I/O"
ban_import "$CORE/store/audit.rs" 'super' 'fsio'                 "store/audit.rs is a record shape; it must not do I/O"
# The domain receives resolved roots and configuration (R3).
ban "$CORE"          'std::env::|home_dir|current_dir'           "the core crate must not read the environment or discover paths"
ban_import "$CORE"   'std' 'env'                                 "the core crate must not read the environment or discover paths"
# Composition does not know about parsing, dispatch or rendering (R2).
ban "$CLI/app.rs"    'crate::(cli|commands|output|audit_middleware)' "app.rs (composition) must not import surface modules"
ban_import "$CLI/app.rs" 'crate' 'cli|commands|output|audit_middleware' "app.rs (composition) must not import surface modules"
# Rendering is derived from payloads only; it never reaches back up (R1).
ban "$CLI/output"    'crate::(app|cli|commands|audit_middleware)'    "output must not import app, cli, commands or audit_middleware"
ban_import "$CLI/output" 'crate' 'app|cli|commands|audit_middleware' "output must not import app, cli, commands or audit_middleware"
# Dispatch depends on the middleware, never the reverse (R1).
ban "$CLI/audit_middleware.rs" 'crate::(cli|commands)'           "audit_middleware must not import cli or commands"
ban_import "$CLI/audit_middleware.rs" 'crate' 'cli|commands'     "audit_middleware must not import cli or commands"

# Persisted shapes never fabricate a timestamp on read (R16).
ban crates 'serde\(default *= *"[A-Za-z_:]*(now|now_utc)"'    "a persisted timestamp must not default to the current time"

# Retired paths stay retired (R17). List a path here when you delete a module.
for retired in \
  crates/tmpl-cli/src/command.rs \
  crates/tmpl-cli/src/tests/command.rs \
  crates/tmpl-cli-core/src/fsio.rs \
  crates/tmpl-cli-core/src/store.rs \
  crates/tmpl-cli-core/src/tests/fsio.rs \
  crates/tmpl-cli-core/src/tests/store.rs; do
  [[ -e "$retired" ]] && err "retired path exists again: $retired"
done

if [[ "$fail" -eq 0 ]]; then
  echo "dependency-direction: ok"
fi
exit "$fail"
