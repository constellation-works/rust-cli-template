#!/usr/bin/env python3
"""The STD-12 source-style checks that stable rustfmt cannot make.

The checks live in the scripts/source_style/ package, one module per subject;
this file is the entry point. Run through scripts/check-source-style.sh (R12,
R13, R16, R19, R21, R22, R26) and scripts/check-singular-names.sh (R1); each
passes its mode as the first argument. Every tracked or new `.rs` file is read, apart from a file whose
leading comment block says `@generated`, which STD-12 exempts. Each breach is
printed as file:line: rule: message, and any breach exits 1.

- R1 (`names`): a module, source directory or source file name ending in `s`
  fails unless it is allowed. `examples` and `benches` are allowed only as a
  crate-root directory, where Cargo fixes them. `tests` is allowed as the
  crate-root directory and as a module's unit-test directory and module,
  which STD-02 R19 fixes (`<module>/tests/`, `mod tests;`). Module names are
  read from `mod` declarations, inline or with `#[path]`, as well as from
  paths. ALLOWED_SINGULARS holds singulars that end in `s`.
- R12: a blank line separates the module documentation (`//!`, `/*! */` and
  inner attributes) from what follows, the use block from the items around
  it, each top-level item from the next, and each method of an `impl`
  (file-scope or in an inline module) from the member before and after it.
  Doc comments, comments and attributes belong to the item they precede, and
  a comment inside a use block does not split it.
- R13: no line is longer than 100 characters, the width R11 and R13 fix;
  `rustfmt.toml` must not set `max_width` to anything else. A line that lies
  within one string literal (plain, byte, C or raw, maybe followed by the
  punctuation that closes its statement) or is one URL (`scheme://…`, bare,
  in `<…>` or after a comment marker) is exempt.
- R16: a `use` imports from one module: its brace group holds no `::`. A `use`
  is found after an attribute on the same line or after a block comment.
- R19: a `#[rustfmt::skip]` has a comment (`//` or `/* */`, not a doc comment)
  on the line before it.
- R21: a `struct`, `enum` or `union` is followed at once by its impl blocks:
  inherent impls, then impls of standard-library traits, then impls of any
  other trait, with no other item between the type and any of them. Read per
  scope (the file, or an inline `mod`), for the impls of the type's bare name.
  An impl whose self type this scope does not define (`u32`, or a path such
  as `cw_base::Error`) belongs to the types of the scope that its trait's
  generics name (`impl From<Title> for String` belongs to `Title`), and to
  none otherwise. A trait is the standard library's when its path starts with
  `std`, `core` or `alloc`, or is the bare or `module::`-qualified name of a
  trait in STD_TRAITS.
- R22: the members of an inherent impl run in groups: associated consts and
  types; functions with no receiver, those returning `Self` (or the impl's
  type) first; `with_*` functions, whatever their receiver; `&self` methods;
  `&mut self` methods; `self` by value. Inside a group of functions, those
  with a `pub` visibility of any kind come before the private ones. Only the
  members of an impl written over several lines are read.
- R26: no path contains `super::super` (also `super::{super::…}`), read from
  the code with comments and string contents blanked.

Known limits, left to review:
- R1 reads names, so it cannot tell an irregular plural (`children`) from a
  singular, nor a plural from a singular ending in `s` outside the allow-list.
- R16 also reads a `use` written inside a macro's tokens (`stringify!`) as an
  import.
- R19 checks that the comment is there, not that it gives a real reason.
- R19 finds `rustfmt::skip` as written; a spelling with a comment inside the
  path (`#[rustfmt /* … */ :: skip]`) is not found.
- R21 and R22 read an item by its written head, so an item a macro expands
  into is not seen, and a type alias or trait stands between nothing. A
  trait named like a standard one (a project `Error` trait) counts as the
  standard library's. R22 reads a method's receiver from its first parameter
  and a constructor from `Self` or the impl's type in the return type; a
  `Self` that only appears inside a larger type (`Vec<Self>`) counts too.
  Where a group holds both constructors and other functions, R22 puts
  constructors first and, inside each, `pub` before private.
"""

import os
import re
import subprocess
import sys
from pathlib import Path

# `python3 -I` leaves the script's directory off sys.path, so add it for the package,
# and write no bytecode cache into the repository.
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))

from source_style.blank_line import check_blank_lines  # noqa: E402
from source_style.deep_super import check_deep_super  # noqa: E402
from source_style.item_order import check_item_order  # noqa: E402
from source_style.lexer import lex, leading_comments_say_generated  # noqa: E402
from source_style.naming import check_names  # noqa: E402
from source_style.skip import check_skips  # noqa: E402
from source_style.use_group import check_use_groups  # noqa: E402
from source_style.width import WIDTH, check_width  # noqa: E402


def tracked_sources():
    listing = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", "*.rs"],
        check=True,
        capture_output=True,
    ).stdout.decode()
    return sorted(
        path for path in set(listing.split("\0"))
        if path and not path.startswith("target/") and os.path.exists(path)
    )


def configured_width():
    """The max_width rustfmt.toml sets, or None when it sets none."""
    if not os.path.exists("rustfmt.toml"):
        return None
    with open("rustfmt.toml", encoding="utf-8") as handle:
        for raw in handle:
            found = re.match(r"\s*max_width\s*=\s*(\S+)", raw)
            if found:
                return found.group(1)
    return None


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "style"
    breaches = []

    def report(path, row, rule, message):
        where = f"{path}:{row}" if row else path
        breaches.append(f"{where}: {rule}: {message}")

    sources = []
    for path in tracked_sources():
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
        lines, literals = lex(text[:-1] if text.endswith("\n") else text)
        if leading_comments_say_generated(lines):
            continue
        sources.append((path, lines, literals))

    if mode == "names":
        check_names([path for path, _, _ in sources], report)
        label = "singular-names"
        rules = "R1"
    else:
        width = configured_width()
        if width is not None and width != str(WIDTH):
            report("rustfmt.toml", 0, "R13", f"max_width is {width}; STD-12 R11 fixes it at {WIDTH}")
        for path, lines, literals in sources:
            check_blank_lines(path, lines, report)
            check_width(path, lines, literals, report)
            check_use_groups(path, lines, report)
            check_skips(path, lines, report)
            check_item_order(path, lines, 0, len(lines), 0, report)
            check_deep_super(path, lines, report)
        label = "source-style"
        rules = "R12, R13, R16, R19, R21, R22, R26"

    if breaches:
        print("\n".join(breaches), file=sys.stderr)
        print(f"{label}: {len(breaches)} breach(es) of STD-12 ({rules})", file=sys.stderr)
        return 1
    print(f"{label}: ok ({len(sources)} files)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
