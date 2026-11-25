#!/usr/bin/env bash
# scripts/check-dependency-direction.sh — STD-02 R1–R6, R16, R17 for this workspace.
#
# Two layers of checks, neither of which compiles, so the gate runs in
# `make ci-fast`:
#
#   1. Crate edges, read from Cargo's own package graph
#      (`cargo metadata --no-deps --locked`), which names each dependency by
#      its package, so a quoted key or a renamed dependency
#      (`files = { package = "cw-fs" }`) is seen for what it is. Every
#      workspace crate needs a policy below. A normal or build dependency may
#      name only the internal crates in ALLOWED and the cw-kit crates (`cw-*`,
#      from the satellites repository) in KIT, and never an external crate in
#      BANNED. A dev-dependency may name the kit crates in KIT and KIT_DEV;
#      other dev edges are free. A crate without a policy fails the check, so
#      a new member forces a decision. Needs cargo and jq.
#   2. Module edges inside a crate, as grep and grouped-import bans over
#      non-test sources.
#
# The layer order is written down in ARCHITECTURE.md; change it and this
# script in the same commit (STD-02 R6).
set -euo pipefail
cd "$(dirname "$0")/.."

PREFIX="tmpl-cli"
KIT_PREFIX="cw"
fail=0
err() { echo "dependency-direction: $*" >&2; fail=1; }

# --- 1. crate policies -------------------------------------------------------
# policy <crate>  -> sets ALLOWED (internal deps), KIT (cw-kit deps),
# KIT_DEV (cw-kit crates allowed as dev-dependencies only) and BANNED
# (external deps)
policy() {
  case "$1" in
    tmpl-cli-store)
      ALLOWED=""
      # The one error type (cw-base), file access for the data directory and
      # the audit log (cw-fs), and the notes database (cw-sqlite). The store
      # is the only crate that names cw-fs or cw-sqlite.
      KIT="cw-base cw-fs cw-sqlite"
      # Test isolation, for its unit tests' private temp roots.
      KIT_DEV="cw-testkit"
      # Persistence never parses arguments, draws on a terminal or installs a
      # log subscriber: those belong to surfaces. It reaches files only
      # through cw-fs and the database only through cw-sqlite, which
      # re-exports rusqlite, so it names no file or SQLite crate itself.
      BANNED="clap tracing-subscriber anstream anstyle termcolor terminal_size crossterm owo-colors colored tempfile rustix rusqlite libsqlite3-sys"
      ;;
    tmpl-cli-core)
      ALLOWED="tmpl-cli-store"
      # The one error type only: the apps reach files through the store.
      KIT="cw-base"
      # Test isolation, for its tests' private temp roots.
      KIT_DEV="cw-testkit"
      # The apps reach files only through the store, so they need no file or
      # process crate either.
      BANNED="clap tracing-subscriber anstream anstyle termcolor terminal_size crossterm owo-colors colored tempfile rustix rusqlite libsqlite3-sys"
      ;;
    tmpl-cli)
      # The binary names only the core; the core re-exports the record shapes
      # it renders and the one error type (cw-base's, as tmpl_cli_core::Error).
      ALLOWED="tmpl-cli-core"
      KIT=""
      # Test isolation: the temp root, the isolated child and goldens.
      KIT_DEV="cw-testkit"
      # SQLite belongs to the store alone.
      BANNED="rusqlite libsqlite3-sys"
      ;;
    *) return 1 ;;
  esac
}

contains() { # contains <word> <space-separated list>
  local word="$1" item
  for item in $2; do [[ "$item" == "$word" ]] && return 0; done
  return 1
}

# check_crate_edges: layer 1, on Cargo's package graph. Exits 2 when the
# graph cannot be read, so a broken manifest never passes as "no edges".
check_crate_edges() {
  local tool metadata members edges crate kind dep
  for tool in cargo jq; do
    if ! command -v "$tool" >/dev/null 2>&1; then
      echo "dependency-direction: cannot run: $tool is not installed" >&2
      exit 2
    fi
  done
  if ! metadata=$(cargo metadata --format-version 1 --no-deps --locked 2>/dev/null); then
    echo "dependency-direction: cannot run: cargo metadata failed" >&2
    exit 2
  fi
  # One line per workspace package: its name.
  members=$(jq -er '.packages[].name' <<<"$metadata") || {
    echo "dependency-direction: cannot run: cargo metadata gave no package list" >&2
    exit 2
  }
  # One line per edge: "<crate> <kind> <package>", kind normal, build or dev.
  edges=$(jq -er '.packages[] | .name as $crate
    | .dependencies[] | "\($crate) \(.kind // "normal") \(.name)"' <<<"$metadata") || edges=""
  while IFS= read -r crate; do
    policy "$crate" || err "crate '$crate' has no policy; add one here and a row in ARCHITECTURE.md"
  done <<<"$members"
  while read -r crate kind dep; do
    [[ -n "$crate" ]] || continue
    policy "$crate" || continue
    if [[ "$kind" == "dev" ]]; then
      if [[ "$dep" == "$KIT_PREFIX"-* ]]; then
        contains "$dep" "$KIT $KIT_DEV" ||
          err "$crate must not depend on kit crate $dep, even for tests (allowed: ${KIT:-none} ${KIT_DEV})"
      fi
    elif [[ "$dep" == "$PREFIX" || "$dep" == "$PREFIX"-* ]]; then
      contains "$dep" "$ALLOWED" || err "$crate must not depend on internal crate $dep"
    elif [[ "$dep" == "$KIT_PREFIX"-* ]]; then
      contains "$dep" "$KIT" || err "$crate must not depend on kit crate $dep (allowed: ${KIT:-none})"
    elif contains "$dep" "$BANNED"; then
      err "$crate must not depend on $dep (surface crate in the domain)"
    fi
  done <<<"$edges"
}

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

STORE=crates/tmpl-cli-store/src
CORE=crates/tmpl-cli-core/src
CLI=crates/tmpl-cli/src

# Store: no code here names a file, process or network primitive; it reaches
# the data directory and the audit log through the kit's cw-fs and the notes
# database through the kit's cw-sqlite. driver/ sets each mechanism up and
# returns its handle; repository/ holds each record's use of it (its SQL and
# row mapping; the audit repository's line.rs converts the audit line's bytes);
# contract/ holds shapes that know nothing of how they are stored and hold no
# rule (R4).
ban "$STORE"         'std::(fs|net|process)|tempfile::|rustix::'           "the store reaches files through cw_fs"
ban_import "$STORE"  'std' 'fs|net|process'                                "the store reaches files through cw_fs"
ban "$STORE/repository/audit/line.rs" 'cw_fs::|cw_sqlite\b'             "repository/audit/line.rs converts bytes; it must not do I/O"
ban "$STORE/contract" 'crate::(driver|repository|store|Store)|super::super::|cw_fs::|cw_sqlite\b|rusqlite\b' "contract/ holds record shapes; they must not import the driver, repository, the store, file access or SQLite"
ban_import "$STORE/contract" 'crate' 'driver|repository|store|Store'       "contract/ holds record shapes; they must not import the driver, repository or the store"
ban "$STORE/contract" 'fn check_|now_utc|SystemTime'                      "contract/ holds shapes only: no checks across records and no clock"
[[ -e "$STORE/db" || -e "$STORE/db.rs" ]] && err "store/db: each record's mapping lives in its repository (the audit line in repository/audit/line.rs)"
[[ -e "$STORE/driver/fs" || -e "$STORE/driver/fs.rs" ]] && err "store/driver/fs: there is no file driver; a file record's repository uses cw_fs directly"
# Module names are singular: one record's module is named for the record.
[[ -e "$STORE/contracts" ]] && err "store/contracts: the record shapes live in contract/"
[[ -e "$CORE/apps" ]] && err "core/apps: the apps live in app/"
[[ -e "$CLI/commands" ]] && err "cli/commands: the command handlers live in command/"
compgen -G "$CORE/app/*/rules.rs" >/dev/null && err "core/app/<app>/rules.rs: an app's rules live in rule.rs"
compgen -G "$CORE/app/*/rules" >/dev/null && err "core/app/<app>/rules/: an app's rules live in rule.rs or rule/"
[[ -e "$STORE/repository/notes.rs" || -e "$STORE/repository/notes" ]] && err "store/repository/notes: the note repository is repository/note.rs"
# rusqlite is named only through the kit's re-export (cw_sqlite::rusqlite),
# never as a crate of its own.
ban "$STORE"         '(^|[^:[:alnum:]_])rusqlite::|extern crate rusqlite'  "the store names rusqlite only through cw_sqlite's re-export"
# A statement that changes the schema appears only in driver/sqlite/schema/:
# one module per table and the one ordered list of steps.
schema_hits=$(grep -rnHiE --include='*.rs' --exclude-dir=tests \
    '\b(create|alter|drop)[[:space:]]+(table|index|view|trigger)\b|pragma[[:space:]]+user_version|Step::(sql|function)' \
    "$STORE" 2>/dev/null | grep -v "^$STORE/driver/sqlite/schema/" | grep -vE '^[^:]+:[0-9]+:[[:space:]]*//' || true)
if [[ -n "$schema_hits" ]]; then
  echo "$schema_hits" >&2
  err "a statement that changes the schema belongs in driver/sqlite/schema/"
fi
# A table's module exports named steps; only schema/mod.rs lists them, one by
# one, so a later step for an earlier table goes at the end (STD-03 R23).
list_hits=$(grep -rnHE --include='*.rs' '&\[(cw_sqlite::)?Step\]|\[(cw_sqlite::)?Step;' \
    "$STORE/driver/sqlite/schema" 2>/dev/null | grep -v "^$STORE/driver/sqlite/schema/mod.rs:" || true)
if [[ -n "$list_hits" ]]; then
  echo "$list_hits" >&2
  err "a table's schema module exports named steps; only driver/sqlite/schema/mod.rs lists them"
fi
[[ -e "$STORE/driver/sqlite/schema/mod.rs" ]] || err "store/driver/sqlite/schema/mod.rs: the one ordered list of schema steps is missing"
# No mechanism in the store's public surface: a public item names no
# rusqlite or cw_sqlite type. (A pub(crate) item may.)
ban "$STORE"         '^[[:space:]]*pub[[:space:]]+(fn|struct|enum|type|use|const|static|trait)[^;{]*(rusqlite|cw_sqlite|Connection|Transaction|Reader|\bDb\b)' "the store's public surface names no rusqlite or cw_sqlite type"
# The store receives a resolved root (R3).
ban "$STORE"         'std::env\b|home_dir|current_dir'                      "the store crate must not read the environment or discover paths"
ban_import "$STORE"  'std' 'env'                                           "the store crate must not read the environment or discover paths"

# Core: apps, runtime and composition. Nothing in the core touches a file or
# a process; it reaches the data directory through the store (R5).
ban "$CORE"          'std::(fs|net|process)|tempfile::|rustix::|cw_fs\b|cw_sqlite\b|rusqlite\b'  "the core reaches files only through tmpl-cli-store"
# The record-access seam: the core reaches a record only through
# snapshot.<record>() and tx.<record>(), never a repository by its path.
ban "$CORE"          'tmpl_cli_store::repository\b'                       "the core reaches a record through snapshot.<record>() or tx.<record>(), never tmpl_cli_store::repository"
ban_import "$CORE"   'tmpl_cli_store' 'repository'                         "the core reaches a record through snapshot.<record>() or tx.<record>(), never tmpl_cli_store::repository"
ban_import "$CORE"   'std' 'fs|net|process'                                "the core reaches files only through tmpl-cli-store"
# Only composition reads the host: environment variables and paths (R3).
for path in "$CORE"/*; do
  name=$(basename "$path")
  case "$name" in composition.rs | tests) continue ;; esac
  ban "$path" 'std::env\b|home_dir|current_dir'                             "core/$name: only composition.rs reads the environment or discovers paths"
  ban_import "$path" 'std' 'env'                                           "core/$name: only composition.rs reads the environment or discovers paths"
done
# The runtime sits below the apps; apps depend on it, never the reverse (R1).
ban "$CORE/runtime"  'crate::app'                                         "runtime must not import an app"
ban_import "$CORE/runtime" 'crate' 'app'                                  "runtime must not import an app"
# One chokepoint (R24): only runtime/exec.rs starts the runtime and the audit
# guard. An app's exec forwards to it and supplies its parts through `App`.
ban "$CORE/app"     'AuditGuard|Runtime::start'                           "an app runs through runtime::exec; it must not start the runtime or the audit guard"
for app in "$CORE"/app/*/mod.rs; do
  [[ -e "$app" ]] || continue
  grep -q 'runtime::exec::<' "$app" || err "${app#"$CORE"/}: exec must forward to runtime::exec"
done
# There is no shared app: code two apps need goes to the lower app or the runtime.
for shared in common shared util; do
  [[ -e "$CORE/app/$shared" || -e "$CORE/app/$shared.rs" ]] && err "core/app/$shared: put shared code in the lower app or in runtime/"
done
# An app's rules are pure: no store, no runtime, no composition (R5).
for rules in "$CORE"/app/*/rule.rs "$CORE"/app/*/rule; do
  [[ -e "$rules" ]] || continue
  ban "$rules" 'crate::(runtime|composition)|Store\b|Snapshot|Tx\b'      "${rules#"$CORE"/}: rules are pure functions over record shapes"
  ban_import "$rules" 'crate' 'runtime|composition'                       "${rules#"$CORE"/}: rules are pure functions over record shapes"
done

# Binary: argument declarations, the mapping to an app, rendering (R2). It
# names only the core, and its handlers open no store and read no environment.
ban "crates/tmpl-cli" 'tmpl_cli_store|cw_(base|fs|sqlite)\b|rusqlite\b'    "the binary names only tmpl-cli-core"
ban "$CLI/command"  'std::(env|fs)|Store\b|Runtime|Ctx\b'               "a command handler maps arguments to an app command; it reads no environment and opens no store"
ban_import "$CLI/command" 'std' 'env|fs'                                  "a command handler maps arguments to an app command; it reads no environment and opens no store"
# Rendering is derived from payloads only; it never reaches back up (R1).
ban "$CLI/output"    'crate::(cli|command)'                                "output must not import cli or command"
ban_import "$CLI/output" 'crate' 'cli|command'                             "output must not import cli or command"

# rusqlite is named only inside tmpl-cli-store, tests included: another
# crate's test that needs hand-made store state uses the store's
# `test-support` feature.
sqlite_hits=$(grep -rnHE --include='*.rs' '\b(rusqlite|cw_sqlite)\b' crates 2>/dev/null \
    | grep -v '^crates/tmpl-cli-store/' | grep -vE '^[^:]+:[0-9]+:[[:space:]]*//' || true)
if [[ -n "$sqlite_hits" ]]; then
  echo "$sqlite_hits" >&2
  err "rusqlite and cw_sqlite are named only inside tmpl-cli-store"
fi

# The kit's test isolation is for tests only: no non-test source names it.
ban crates 'cw_testkit'                                                  "cw-testkit is a dev-dependency: only test code names it"

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
  crates/tmpl-cli-store/src/db/tests/fsio.rs \
  crates/tmpl-cli-store/src/db/format.rs \
  crates/tmpl-cli-store/src/db/tests/format.rs; do
  [[ -e "$retired" ]] && err "retired path exists again: $retired"
done

# Crate edges last: the module bans above run even where Cargo cannot.
check_crate_edges

if [[ "$fail" -eq 0 ]]; then
  echo "dependency-direction: ok"
fi
exit "$fail"
