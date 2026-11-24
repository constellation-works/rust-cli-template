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
    tmpl-cli-common)
      # The foundation names no other crate of the workspace: no terminal,
      # parser or log-subscriber crate, and no byte format (serde_json
      # belongs to whoever persists or renders). tempfile and rustix serve
      # its fs/ module only (module rules below).
      ALLOWED=""
      BANNED="clap tracing-subscriber serde_json anstream anstyle termcolor terminal_size crossterm owo-colors colored"
      ;;
    tmpl-cli-store)
      ALLOWED="tmpl-cli-common"
      # Persistence never parses arguments, draws on a terminal or installs a
      # log subscriber: those belong to surfaces. It reaches files only
      # through the foundation's fs module, so it needs no file crate.
      BANNED="clap tracing-subscriber anstream anstyle termcolor terminal_size crossterm owo-colors colored tempfile rustix"
      ;;
    tmpl-cli-core)
      ALLOWED="tmpl-cli-common tmpl-cli-store"
      # The apps reach files only through the store, so they need no file or
      # process crate either.
      BANNED="clap tracing-subscriber anstream anstyle termcolor terminal_size crossterm owo-colors colored tempfile rustix"
      ;;
    tmpl-cli)
      # The binary names only the core; the core re-exports the record shapes
      # it renders and the one error type.
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

COMMON=crates/tmpl-cli-common/src
STORE=crates/tmpl-cli-store/src
CORE=crates/tmpl-cli-core/src
CLI=crates/tmpl-cli/src

# Foundation: the error type and file access. Only fs/ names a file, process
# or clock; error/ is a shape and never imports fs/. fs/ is mechanism (R4):
# the foundation names no other crate, so it cannot know a record. Nothing
# here reads the environment (R3). It is grouped by subject, one folder each: no
# file beside lib.rs, and no folder named for no responsibility.
ban "$COMMON"        'std::env::|home_dir|current_dir'                     "the foundation reads no environment and discovers no path"
ban_import "$COMMON" 'std' 'env'                                           "the foundation reads no environment and discovers no path"
for path in "$COMMON"/*; do
  name=$(basename "$path")
  case "$name" in
    lib.rs | fs) continue ;;
    *.rs) err "common/src/$name: the foundation is grouped by subject; put it in a folder (error/, fs/)"; continue ;;
    util | utils | helpers | misc | shared | types | models) err "common/src/$name: name the folder for what it owns"; continue ;;
  esac
  ban "$path" 'std::(fs|net|process)|tempfile::|rustix::|crate::fs\b'     "common/$name holds shapes; only fs/ touches a file"
  ban_import "$path" 'std' 'fs|net|process'                                "common/$name holds shapes; only fs/ touches a file"
  ban_import "$path" 'crate' 'fs'                                          "common/$name holds shapes; only fs/ touches a file"
  ban "$path" 'fn check_|now_utc|SystemTime'                               "common/$name holds shapes only: no checks across records and no clock"
done

# Store: no code here names a file, process or network primitive; it reaches
# the data directory through the foundation's fs module. db/ holds the file
# names and converts bytes; contracts are shapes that know nothing of how
# they are stored and hold no rule (R4).
ban "$STORE"         'std::(fs|net|process)|tempfile::|rustix::'           "the store reaches files through tmpl_cli_common::fs"
ban_import "$STORE"  'std' 'fs|net|process'                                "the store reaches files through tmpl_cli_common::fs"
for file in format audit; do
  ban "$STORE/db/$file.rs" 'tmpl_cli_common::fs'                           "db/$file.rs converts bytes; it must not do I/O"
  ban_import "$STORE/db/$file.rs" 'tmpl_cli_common' 'fs'                   "db/$file.rs converts bytes; it must not do I/O"
done
ban "$STORE/contracts" 'crate::(db|repository|store|Store)|super::super::|tmpl_cli_common::fs' "contracts are record shapes; they must not import db, repository, the store or file access"
ban_import "$STORE/contracts" 'crate' 'db|repository|store|Store'           "contracts are record shapes; they must not import db, repository or the store"
ban_import "$STORE/contracts" 'tmpl_cli_common' 'fs'                        "contracts are record shapes; they must not import file access"
ban "$STORE/contracts" 'fn check_|now_utc|SystemTime'                      "contracts hold shapes only: no checks across records and no clock"
[[ -e "$STORE/db/fsio.rs" ]] && err "store/db/fsio.rs: file access lives in tmpl-cli-common/src/fs"
# The store receives a resolved root (R3).
ban "$STORE"         'std::env\b|home_dir|current_dir'                      "the store crate must not read the environment or discover paths"
ban_import "$STORE"  'std' 'env'                                           "the store crate must not read the environment or discover paths"

# Core: apps, runtime and composition. Nothing in the core touches a file or
# a process; it reaches the data directory through the store (R5).
ban "$CORE"          'std::(fs|net|process)|tempfile::|rustix::|tmpl_cli_common::fs' "the core reaches files only through tmpl-cli-store"
ban_import "$CORE"   'std' 'fs|net|process'                                "the core reaches files only through tmpl-cli-store"
ban_import "$CORE"   'tmpl_cli_common' 'fs'                                "the core reaches files only through tmpl-cli-store"
# Only composition reads the host: environment variables and paths (R3).
for path in "$CORE"/*; do
  name=$(basename "$path")
  case "$name" in composition.rs | tests) continue ;; esac
  ban "$path" 'std::env\b|home_dir|current_dir'                             "core/$name: only composition.rs reads the environment or discovers paths"
  ban_import "$path" 'std' 'env'                                           "core/$name: only composition.rs reads the environment or discovers paths"
done
# The runtime sits below the apps; apps depend on it, never the reverse (R1).
ban "$CORE/runtime"  'crate::apps'                                         "runtime must not import an app"
ban_import "$CORE/runtime" 'crate' 'apps'                                  "runtime must not import an app"
# One chokepoint (R24): only runtime/exec.rs starts the runtime and the audit
# guard. An app's exec forwards to it and supplies its parts through `App`.
ban "$CORE/apps"     'AuditGuard|Runtime::start'                           "an app runs through runtime::exec; it must not start the runtime or the audit guard"
for app in "$CORE"/apps/*/mod.rs; do
  [[ -e "$app" ]] || continue
  grep -q 'runtime::exec::<' "$app" || err "${app#"$CORE"/}: exec must forward to runtime::exec"
done
# There is no shared app: code two apps need goes to the lower app or the runtime.
for shared in common shared util; do
  [[ -e "$CORE/apps/$shared" || -e "$CORE/apps/$shared.rs" ]] && err "core/apps/$shared: put shared code in the lower app or in runtime/"
done
# An app's rules are pure: no store, no runtime, no composition (R5).
for rules in "$CORE"/apps/*/rules.rs "$CORE"/apps/*/rules; do
  [[ -e "$rules" ]] || continue
  ban "$rules" 'crate::(runtime|composition)|Store\b|Snapshot|Tx\b'      "${rules#"$CORE"/}: rules are pure functions over record shapes"
  ban_import "$rules" 'crate' 'runtime|composition'                       "${rules#"$CORE"/}: rules are pure functions over record shapes"
done

# Binary: argument declarations, the mapping to an app, rendering (R2). It
# names only the core, and its handlers open no store and read no environment.
ban "crates/tmpl-cli" 'tmpl_cli_(store|common)'                            "the binary names only tmpl-cli-core"
ban "$CLI/commands"  'std::(env|fs)|Store\b|Runtime|Ctx\b'               "a command handler maps arguments to an app command; it reads no environment and opens no store"
ban_import "$CLI/commands" 'std' 'env|fs'                                  "a command handler maps arguments to an app command; it reads no environment and opens no store"
# Rendering is derived from payloads only; it never reaches back up (R1).
ban "$CLI/output"    'crate::(cli|commands)'                                "output must not import cli or commands"
ban_import "$CLI/output" 'crate' 'cli|commands'                             "output must not import cli or commands"

# Persisted shapes never fabricate a timestamp on read (R16).
ban crates 'serde\(default *= *"[A-Za-z_:]*(now|now_utc)"'    "a persisted timestamp must not default to the current time"

# Retired paths stay retired (R17). List a path here when you delete a module.
for retired in \
  crates/tmpl-cli/src/app.rs \
  crates/tmpl-cli/src/audit_middleware.rs \
  crates/tmpl-cli/src/tests/app.rs \
  crates/tmpl-cli/src/tests/audit_middleware.rs \
  crates/tmpl-cli-core/src/error.rs \
  crates/tmpl-cli-core/src/note.rs \
  crates/tmpl-cli-core/src/query.rs \
  crates/tmpl-cli-core/src/store \
  crates/tmpl-cli-core/src/tests/note.rs \
  crates/tmpl-cli-core/src/tests/query.rs \
  crates/tmpl-cli-core/tests/store.rs \
  crates/tmpl-cli/src/command.rs \
  crates/tmpl-cli/src/tests/command.rs \
  crates/tmpl-cli-core/src/fsio.rs \
  crates/tmpl-cli-core/src/store.rs \
  crates/tmpl-cli-core/src/tests/fsio.rs \
  crates/tmpl-cli-core/src/tests/store.rs \
  crates/tmpl-cli-core/src/runtime/errors.rs \
  crates/tmpl-cli-store/src/db/tests/fsio.rs; do
  [[ -e "$retired" ]] && err "retired path exists again: $retired"
done

if [[ "$fail" -eq 0 ]]; then
  echo "dependency-direction: ok"
fi
exit "$fail"
