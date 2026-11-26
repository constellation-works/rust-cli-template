#!/usr/bin/env bash
# scripts/check-source-style.sh — the STD-12 source-style checks that rustfmt
# on the stable toolchain cannot make: R12 blank lines, R13 line width (fixed
# at 100), R16 one module per use, R19 the rustfmt::skip reason, R21 a type
# beside its impls, R22 the order of an inherent impl's members and R26 no
# `super::super`.
#
# The checks, the files they read and their known limits are in
# scripts/check-source-style.py, which this runs in its `style` mode.
set -euo pipefail
cd "$(dirname "$0")/.."
exec python3 -I scripts/check-source-style.py style
