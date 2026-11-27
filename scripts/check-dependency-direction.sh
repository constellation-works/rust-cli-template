#!/usr/bin/env bash
# scripts/check-dependency-direction.sh — STD-02 R1–R6, R16, R17, R35, R39 for this workspace.
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
#   3. No import cycle between units (STD-02 R6, R39):
#      `scripts/check-module-cycles.sh` checks each crate's top-level modules
#      and the modules of each top-level directory. Cargo refuses a cyclic
#      crate graph itself.
#   4. No `deny(warnings)` in any Rust source, tests included (STD-02 R35):
#      CI denies warnings with `-D warnings`, and a source-level deny turns a
#      new toolchain's new lint into a broken build for everyone.
#
# File access, the process environment and spawning a process are refused
# by clippy, not here: `clippy.toml` lists the std paths (and
# cw_app::Env::capture) and the workspace denies `clippy::disallowed_methods`
# and `clippy::disallowed_types`, which see through `use std::env;` and every
# alias. What clippy cannot state stays here: crate edges, `crate::` imports,
# module-to-module rules, `std::net` and names that are not paths.
#
# The layer order is written down in ARCHITECTURE.md; change it and this
# script in the same commit (STD-02 R6).
set -euo pipefail
cd "$(dirname "$0")/.."

PREFIX="tmpl-cli"
KIT_PREFIX="cw"
fail=0
err() { echo "dependency-direction: $*" >&2; fail=1; }

# Decide emptiness from Cargo (not a path glob): a workspace with no packages
# yet has no crate or module edge to check.
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
package_count=$(jq -er '.packages | length' <<<"$metadata") || {
  echo "dependency-direction: cannot run: cargo metadata gave no package list" >&2
  exit 2
}
if [[ "$package_count" -eq 0 ]]; then
  echo "dependency-direction: workspace has no packages yet; nothing to check"
  exit 0
fi

# --- 1. crate policies -------------------------------------------------------
# policy <crate>  -> sets ALLOWED (internal deps), KIT (cw-kit deps),
# KIT_DEV (cw-kit crates allowed as dev-dependencies only) and BANNED
# (external deps)
policy() {
  case "$1" in
    # crates:begin
    tmpl-cli-store)
      ALLOWED=""
      # The one error type (cw-base), file access for the data directory
      # (cw-fs), and the database (cw-sqlite). The store is the only crate
      # that names cw-fs or cw-sqlite. It holds no audit code: the audit log
      # is cw-app's, named by the core.
      KIT="cw-base cw-fs cw-sqlite"
      # Test isolation, for its unit tests' private temp roots.
      KIT_DEV="cw-testkit"
      # Persistence never parses arguments, draws on a terminal or installs a
      # log subscriber: those belong to surfaces. It reaches files only
      # through cw-fs and the database only through cw-sqlite, which
      # re-exports rusqlite, so it names no file or SQLite crate itself. It
      # receives a resolved root, so it takes no path-finding crate either.
      BANNED="clap tracing-subscriber anstream anstyle termcolor terminal_size crossterm owo-colors colored tempfile rustix rusqlite libsqlite3-sys dirs dirs-next directories home"
      ;;
    tmpl-cli-core)
      ALLOWED="tmpl-cli-store"
      # The one error type (cw-base) and the command path (cw-app: the
      # chokepoint, the captured environment and the audit log). The apps
      # reach the records through the store.
      KIT="cw-base cw-app"
      # Test isolation, for its tests' private temp roots, and cw-fs for the
      # capacity the kit's audit reader takes (cw-app does not re-export it).
      KIT_DEV="cw-testkit cw-fs"
      # The apps reach files only through the store, so they need no file or
      # process crate either, and the data directory is the kit's, so no
      # path-finding crate.
      BANNED="clap tracing-subscriber anstream anstyle termcolor terminal_size crossterm owo-colors colored tempfile rustix rusqlite libsqlite3-sys dirs dirs-next directories home"
      ;;
    tmpl-cli)
      # The binary names only the core; the core re-exports the record shapes
      # it renders, the one error type (cw-base's, as tmpl_cli_core::Error)
      # and what a surface needs of cw-app (Env, Origin, the runtime). The
      # command line's mechanism is cw-cli's: the terminal decision, the
      # sink, error rendering, logging and grouped help. Its second surface,
      # the MCP server, is cw-mcp's. No other crate names cw-cli or cw-mcp
      # (they are in no other KIT list).
      ALLOWED="tmpl-cli-core"
      KIT="cw-cli cw-mcp"
      # Test isolation: the temp root, the isolated child and goldens.
      KIT_DEV="cw-testkit"
      # SQLite belongs to the store alone. Terminal state, colour and the log
      # subscriber are cw-cli's, so the CLI names none of their crates.
      BANNED="rusqlite libsqlite3-sys tracing-subscriber anstream anstyle termcolor terminal_size crossterm owo-colors colored rustix"
      ;;
    # crates:end
    *) return 1 ;;
  esac
}

contains() { # contains <word> <space-separated list>
  local word="$1" item
  for item in $2; do [[ "$item" == "$word" ]] && return 0; done
  return 1
}

# check_crate_edges: layer 1, on Cargo's package graph.
check_crate_edges() {
  local members edges crate kind dep
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

# crates:begin
STORE=crates/tmpl-cli-store/src
CORE=crates/tmpl-cli-core/src
CLI=crates/tmpl-cli/src

# Store: no code here names a file, process or network primitive; it reaches
# the data directory through the kit's cw-fs and the database through the
# kit's cw-sqlite. driver/ sets each mechanism up and returns its handle;
# repository/ holds each record's use of it (its SQL and row mapping);
# contract/ holds shapes that know nothing of how they are stored and hold no
# rule (R4). The audit log is cw-app's: the store holds no audit code.
ban "$STORE"         'std::net|tempfile::|rustix::'                        "the store reaches files through cw_fs and opens no socket"
ban_import "$STORE"  'std' 'net'                                           "the store opens no socket"
ban "$STORE"         'audit\.jsonl|\.audit\.lock|Audit(Event|Status|Line|Guard)|cw_app\b' "the store holds no audit code or file: the audit log is cw-app's"
ban "$STORE/contract" 'crate::(driver|repository|store|Store)|super::super::|cw_fs::|cw_sqlite\b|rusqlite\b' "contract/ holds record shapes; they must not import the driver, repository, the store, file access or SQLite"
ban_import "$STORE/contract" 'crate' 'driver|repository|store|Store'       "contract/ holds record shapes; they must not import the driver, repository or the store"
ban "$STORE/contract" 'fn check_|now_utc|SystemTime'                      "contract/ holds shapes only: no checks across records and no clock"
[[ -e "$STORE/db" || -e "$STORE/db.rs" ]] && err "store/db: each record's mapping lives in its repository"
[[ -e "$STORE/driver/fs" || -e "$STORE/driver/fs.rs" ]] && err "store/driver/fs: there is no file driver; a file record's repository uses cw_fs directly"
# Module names are singular: one record's module is named for the record.
[[ -e "$STORE/contracts" ]] && err "store/contracts: the record shapes live in contract/"
[[ -e "$CORE/apps" ]] && err "core/apps: the apps live in app/"
[[ -e "$CLI/commands" ]] && err "cli/commands: the command handlers live in command/"
compgen -G "$CORE/app/*/rules.rs" >/dev/null && err "core/app/<app>/rules.rs: an app's rules live in rule.rs"
compgen -G "$CORE/app/*/rules" >/dev/null && err "core/app/<app>/rules/: an app's rules live in rule.rs or rule/"
# example:begin
[[ -e "$STORE/repository/notes.rs" || -e "$STORE/repository/notes" ]] && err "store/repository/notes: the note repository is repository/note.rs"
# example:end
# rusqlite is named only through the kit's re-export (cw_sqlite::rusqlite),
# never as a crate of its own.
ban "$STORE"         '(^|[^:[:alnum:]_])rusqlite::|extern crate rusqlite'  "the store names rusqlite only through cw_sqlite's re-export"
# Example, not enforced: "this dependency only in this module". When a crate
# may name a dependency in one module alone (say an HTTP client only in the
# store's driver/http/), Cargo cannot state it, because the edge is the
# crate's; ban the name everywhere else in the crate, as the rusqlite rule
# above and the schema rule below do:
#
#   hits=$(grep -rnHE --include='*.rs' --exclude-dir=tests '\breqwest::' "$STORE" 2>/dev/null \
#     | grep -v "^$STORE/driver/http/" | grep -vE '^[^:]+:[0-9]+:[[:space:]]*//' || true)
#   [[ -n "$hits" ]] && { echo "$hits" >&2; err "reqwest is named only in the store's driver/http/"; }
#
# Example, not a grep: "only this module may spawn a process" is clippy's to
# hold, not this script's. clippy.toml already refuses std::process::Command's
# spawn, output and status everywhere, so the one module that must spawn
# (through the kit's cw_supervise, or std where that is the point) opens with
#
#   #![expect(clippy::disallowed_methods, reason = "the one module that spawns a process: <why>")]
#
# and a module that spawns without it fails `make ci-lint`.
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

# Core: apps and composition. Nothing in the core touches a file or
# a process; it reaches the data directory through the store (R5).
ban "$CORE"          'std::net|tempfile::|rustix::|cw_fs\b|cw_sqlite\b|rusqlite\b'  "the core reaches files only through tmpl-cli-store"
# The record-access seam: the core reaches a record only through
# snapshot.<record>() and tx.<record>(). The store's `repository` module is
# private, so a repository path from the core does not compile: visibility
# holds the seam (STD-02 R6), and no grep repeats it here.
ban_import "$CORE"   'std' 'net'                                           "the core opens no socket"
# One chokepoint (R24): the kit's cw_app::exec. An app's exec forwards to it
# and supplies its parts by implementing cw_app::App; nothing calls an app's
# run itself, and there is no runtime of the template's own.
for app in "$CORE"/app/*/mod.rs; do
  [[ -e "$app" ]] || continue
  grep -q 'cw_app::exec::<' "$app" || err "${app#"$CORE"/}: exec must forward to cw_app::exec"
done
ban crates           '(App|App>)::run\b|cw_app::(Pending|audit::)|AuditLog::new|\.append\('  "a command runs only through cw_app::exec, and only the kit writes the audit log"
# There is no shared app: code two apps need goes to the lower app.
for shared in common shared util; do
  [[ -e "$CORE/app/$shared" || -e "$CORE/app/$shared.rs" ]] && err "core/app/$shared: put shared code in the lower app"
done
# An app's rules are pure: no store, no context, no composition (R5).
for rules in "$CORE"/app/*/rule.rs "$CORE"/app/*/rule; do
  [[ -e "$rules" ]] || continue
  ban "$rules" 'crate::composition|cw_app\b|Ctx\b|Store\b|Snapshot|Tx\b'  "${rules#"$CORE"/}: rules are pure functions over record shapes"
  ban_import "$rules" 'crate' 'composition'                               "${rules#"$CORE"/}: rules are pure functions over record shapes"
done

# Binary: argument declarations, the mapping to an app, rendering (R2). It
# names only the core, and its handlers open no store and read no environment.
ban "crates/tmpl-cli" 'tmpl_cli_store|cw_(base|fs|sqlite|app)\b|rusqlite\b' "the binary names only tmpl-cli-core"
ban "$CLI/command"  'Store\b|Runtime::|start\(|Ctx\b|\bEnv\b'            "a command handler maps arguments to an app command; it receives no environment, opens no store and starts no runtime"
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
retired_paths=(
  crates/tmpl-cli/src/app.rs
  crates/tmpl-cli/src/audit_middleware.rs
  crates/tmpl-cli/src/tests/app.rs
  crates/tmpl-cli/src/tests/audit_middleware.rs
  crates/tmpl-cli-core/src/error.rs
  crates/tmpl-cli-core/src/query.rs
  crates/tmpl-cli-core/src/store
  crates/tmpl-cli-core/src/tests/query.rs
  crates/tmpl-cli-core/tests/store.rs
  crates/tmpl-cli/src/command.rs
  crates/tmpl-cli/src/tests/command.rs
  crates/tmpl-cli-core/src/fsio.rs
  crates/tmpl-cli-core/src/store.rs
  crates/tmpl-cli-core/src/tests/fsio.rs
  crates/tmpl-cli-core/src/tests/store.rs
  crates/tmpl-cli-core/src/runtime/errors.rs
  crates/tmpl-cli-store/src/db/tests/fsio.rs
  crates/tmpl-cli-store/src/db/format.rs
  crates/tmpl-cli-store/src/db/tests/format.rs
  crates/tmpl-cli-core/src/runtime
  crates/tmpl-cli-core/src/test_support.rs
  crates/tmpl-cli-store/src/repository/audit
  crates/tmpl-cli-store/src/contract/audit.rs
  # example:begin
  crates/tmpl-cli-core/src/note.rs
  crates/tmpl-cli-core/src/tests/note.rs
  # example:end
)
for retired in "${retired_paths[@]}"; do
  [[ -e "$retired" ]] && err "retired path exists again: $retired"
done
# crates:end

# Crate edges last: the module bans above run even where Cargo cannot.
check_crate_edges

# The module import graph is acyclic (STD-02 R6, R39).
./scripts/check-module-cycles.sh || fail=1

# No deny(warnings) in source, tests included (STD-02 R35): warnings are
# denied by CI's `-D warnings`, never by the code.
pkg_dirs=()
while IFS= read -r manifest; do
  [[ -n "$manifest" ]] && pkg_dirs+=("$(dirname "$manifest")")
done < <(jq -r '.packages[].manifest_path' <<<"$metadata")
if [[ ${#pkg_dirs[@]} -gt 0 ]]; then
  deny_hits=$(grep -rnHE --include='*.rs' 'deny\([[:space:]]*warnings[[:space:]]*\)' "${pkg_dirs[@]}" 2>/dev/null || true)
  if [[ -n "$deny_hits" ]]; then
    echo "$deny_hits" >&2
    err "deny(warnings) in source: CI denies warnings with -D warnings (STD-02 R35)"
  fi
fi

if [[ "$fail" -eq 0 ]]; then
  echo "dependency-direction: ok"
fi
exit "$fail"
