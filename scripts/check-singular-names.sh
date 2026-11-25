#!/usr/bin/env bash
# scripts/check-singular-names.sh — STD-12 R1: a module, source directory or
# source file is named in the singular.
#
# The check, the names it allows and its known limits are in
# scripts/check-source-style.py, which this runs in its `names` mode.
set -euo pipefail
cd "$(dirname "$0")/.."
exec python3 -I scripts/check-source-style.py names
