#!/bin/sh
# Scan the working copies of tracked paths, excluding the vendored canon.
set -eu
cd "$(dirname "$0")/.."
if git grep -nE 'ORB-[0-9]+' -- . ':(exclude)docs/standards'; then
    echo "error: replace private task IDs in shipped files with public references or placeholders" >&2
    exit 1
else
    status=$?
    [ "$status" -eq 1 ] || exit "$status"
fi
