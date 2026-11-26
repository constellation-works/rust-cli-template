#!/bin/sh
# Scan the working copies of tracked paths, excluding the vendored canon. Outside
# a git work tree git sees no files, so every file under the root is scanned
# instead, skipping .git and target directories and the canon.
set -eu
cd "$(dirname "$0")/.."

# Print each match outside the canon and return 0; return 1 when there is none,
# and grep's own status on an error.
plain_scan() {
    status=0
    found=$(grep -rnE --exclude-dir=.git --exclude-dir=target 'ORB-[0-9]+' .) || status=$?
    [ "$status" -le 1 ] || return "$status"
    # The canon is dropped by path: --exclude-dir would also skip a directory
    # of that name at any depth.
    matches=$(printf '%s\n' "$found" | grep -v '^\./docs/standards/' || true)
    [ -n "$matches" ] || return 1
    printf '%s\n' "$matches"
}

scan() {
    if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        git grep -nE 'ORB-[0-9]+' -- . ':(exclude)docs/standards'
    else
        plain_scan
    fi
}

status=0
hits=$(scan) || status=$?
case "$status" in
0)
    printf '%s\n' "$hits"
    echo "error: replace private task IDs in shipped files with public references or placeholders" >&2
    exit 1
    ;;
1) exit 0 ;;
*) exit "$status" ;;
esac
