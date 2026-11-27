"""R21 and R22: a type's impls follow it in order, and an inherent impl's members run in groups."""

import re

from .blank_line import (
    FN_HEAD,
    IMPL_HEAD,
    MOD_HEAD,
    USE_HEAD,
    attribute_end,
    body_range,
    is_comment_only,
    strip_attributes,
)


STD_TRAITS = {
    "AsMut", "AsRef", "Borrow", "BorrowMut", "Clone", "Copy", "Debug", "Default",
    "Deref", "DerefMut", "Display", "Drop", "Eq", "Error", "Extend", "From",
    "FromIterator", "FromStr", "Hash", "Index", "IndexMut", "Into", "IntoIterator",
    "Iterator", "Ord", "PartialEq", "PartialOrd", "Send", "Sync", "ToOwned",
    "ToString", "TryFrom", "TryInto", "Unpin", "Add", "Sub", "Mul", "Div", "Neg",
    "Not", "Rem", "AddAssign", "SubAssign", "MulAssign", "DivAssign", "BitAnd",
    "BitOr", "BitXor", "Shl", "Shr", "Read", "Write", "Seek", "BufRead", "Future",
    "Termination", "Pointer", "LowerHex", "UpperHex", "Binary", "Octal",
}
VIS = r"(?:pub(?:\s*\([^)]*\))?\s+)?"
TYPE_HEAD = re.compile(r"^" + VIS + r"(struct|enum|union)\s+(?:r#)?([A-Za-z_][A-Za-z0-9_]*)")
CONST_HEAD = re.compile(r"^" + VIS + r"(const|static)\b")
ASSOC_TYPE_HEAD = re.compile(r"^" + VIS + r"(?:default\s+)?type\b")
FN_NAME = re.compile(r"\bfn\s+(?:r#)?([A-Za-z_][A-Za-z0-9_]*)")
PUBLIC = re.compile(r"^pub\b(?!\s*\(\s*(?:self|in\s+self)\s*\))")


class Item:
    """One item at a scope: its kind, name, header text and rows."""

    def __init__(self, kind, name, head, header, start, head_row, end):
        self.kind = kind
        self.name = name
        self.head = head  # the item's head with its attributes stripped
        self.header = header  # the head up to the first `{` or `;`
        self.start = start  # first row, doc comment and attributes included
        self.head_row = head_row
        self.end = end  # last row


def classify(head):
    """The item's kind and name, from the head with attributes stripped."""
    declared = MOD_HEAD.match(head)
    if declared:
        return "mod", declared.group(4)
    if USE_HEAD.match(head):
        return "use", None
    if FN_HEAD.match(head):
        found = FN_NAME.search(head)
        return "fn", found.group(1) if found else None
    if IMPL_HEAD.match(head):
        return "impl", None
    type_def = TYPE_HEAD.match(head)
    if type_def:
        return type_def.group(1), type_def.group(2)
    if CONST_HEAD.match(head):
        return "const", None
    if ASSOC_TYPE_HEAD.match(head):
        return "type", None
    return "other", None


def parse_items(lines, begin, end, depth):
    """The items at `depth` in lines[begin:end], in source order."""
    items = []
    row = begin
    while row < end:
        line = lines[row]
        if line.is_blank() or line.start_depth != depth:
            row += 1
            continue
        if line.has_code and line.code_text.strip().startswith("}") and line.end_depth < depth:
            break
        head_row = None
        cursor = row
        while cursor < end:
            current = lines[cursor]
            if current.is_blank():
                cursor += 1
                continue
            if is_comment_only(current):
                cursor += 1
                continue
            stripped = strip_attributes(current.code_text)
            if stripped == "" and current.code_text.strip().startswith("#"):
                cursor = attribute_end(lines, cursor) + 1
                continue
            head_row = cursor
            break
        if head_row is None:
            break
        head = strip_attributes(lines[head_row].code_text)
        unit_end = head_row
        while unit_end < end:
            current = lines[unit_end]
            if current.end_depth < depth:
                break
            if current.end_depth == depth and depth in current.closes:
                break
            unit_end += 1
        unit_end = min(unit_end, end - 1)
        header = head
        cursor = head_row
        while not re.search(r"[{;]", header) and cursor < unit_end:
            cursor += 1
            header += " " + lines[cursor].code_text.strip()
        header = re.split(r"[{;]", header, maxsplit=1)[0].strip()
        kind, name = classify(head)
        items.append(Item(kind, name, head, header, row, head_row, unit_end))
        row = unit_end + 1
    return items


def body_rows(lines, item, depth):
    """The rows (begin, end) inside the item's braces, when they span lines."""
    found = body_range(lines, item.head_row, item.end, depth)
    if found is None:
        return None
    return found[0], item.end


def angle_end(text, start):
    """The index just past the `>` that closes the `<` at text[start]."""
    level = 0
    flat = text.replace("->", "--")
    for at in range(start, len(flat)):
        if flat[at] == "<":
            level += 1
        elif flat[at] == ">":
            level -= 1
            if level == 0:
                return at + 1
    return len(text)


def impl_target(header):
    """(trait path or None, self-type name or None) of an impl's header."""
    text = re.sub(r"^(?:(?:unsafe|default)\s+)*impl\b\s*", "", header)
    if text.startswith("<"):
        text = text[angle_end(text, 0) :].lstrip()
    text = re.split(r"\bwhere\b", text, maxsplit=1)[0].strip()
    level = 0
    split = None
    flat = text.replace("->", "--")
    for found in re.finditer(r"[<>()\[\]]|\bfor\b(?!\s*<)", flat):
        token = found.group(0)
        if token in "<([":
            level += 1
        elif token in ">)]":
            level -= 1
        elif level == 0:
            split = found
            break
    if split is None:
        trait, self_type = None, text
    else:
        trait, self_type = text[: split.start()].strip(), text[split.end() :].strip()
    path = re.sub(r"^(?:&\s*(?:'[A-Za-z_]+\s+)?(?:mut\s+)?|dyn\s+)+", "", self_type)
    named = re.match(r"((?:[A-Za-z_][A-Za-z0-9_]*\s*::\s*)*)(?:r#)?([A-Za-z_][A-Za-z0-9_]*)", path)
    if not named:
        return trait, None
    prefix = re.sub(r"\s+", "", named.group(1))
    if prefix and prefix.split("::")[0] not in ("crate", "self", "super", "Self"):
        return trait, None  # another crate's type
    return trait, named.group(2)


def is_std_trait(trait):
    path = re.sub(r"\s+", "", re.sub(r"<.*", "", trait)).lstrip("!")
    parts = path.split("::")
    if parts[0] in ("std", "core", "alloc"):
        return True
    if parts[0] in ("crate", "self", "super"):
        return False
    return parts[-1] in STD_TRAITS


def owners(item, defined):
    """The type names in `defined` an impl belongs to.

    An impl belongs to its own type when this scope defines it. An impl for
    another type (`impl From<Title> for String`) belongs to the types of this
    scope that its trait's generics name.
    """
    trait, name = impl_target(item.header)
    if name in defined:
        return {name}
    if trait is None:
        return set()
    return {word for word in re.findall(r"[A-Za-z_][A-Za-z0-9_]*", trait) if word in defined}


def check_adjacency(path, lines, items, report):
    """R21 over one scope's items."""
    defined = {item.name for item in items if item.kind in ("struct", "enum", "union")}
    for position, item in enumerate(items):
        if item.kind not in ("struct", "enum", "union") or item.name is None:
            continue
        run = []
        for follower in items[position + 1 :]:
            if follower.kind != "impl" or item.name not in owners(follower, defined):
                break
            run.append(follower)
        previous = 0
        for follower in run:
            trait, _ = impl_target(follower.header)
            rank = 0 if trait is None else 1 if is_std_trait(trait) else 2
            if rank < previous:
                report(path, follower.head_row + 1, "R21",
                       f"impls of `{item.name}`: inherent, then standard-library traits, "
                       "then the project's")
            previous = max(previous, rank)
        for other in items:
            if other.kind == "impl" and other not in run and item.name in owners(other, defined):
                report(path, other.head_row + 1, "R21",
                       f"this impl of `{item.name}` does not directly follow the type "
                       "and its other impls")


def receiver_group(params):
    """3, 4 or 5 for a `&self`, `&mut self` or `self` first parameter; else None."""
    first = params.split(",", 1)[0] if params else ""
    first = re.sub(r"\s+", " ", first.strip())
    flat = re.sub(r"^&\s*(?:'[A-Za-z_]+\s+)?", "&", first)
    if re.match(r"&mut self$", flat.replace("& mut", "&mut")):
        return 4
    if flat == "&self":
        return 3
    if re.match(r"(mut )?self$", flat):
        return 5
    typed = re.match(r"(?:mut )?self\s*:\s*(.*)$", first)
    if typed:
        kind = typed.group(1)
        if "&mut" in kind.replace("& mut", "&mut"):
            return 4
        if "&" in kind:
            return 3
        return 5
    return None


def parameters(header):
    """The text between the parentheses of a fn header, or None."""
    start = header.find("(", header.find("fn "))
    if start == -1:
        return None, ""
    level = 0
    for at in range(start, len(header)):
        if header[at] == "(":
            level += 1
        elif header[at] == ")":
            level -= 1
            if level == 0:
                return header[start + 1 : at], header[at + 1 :]
    return header[start + 1 :], ""


GROUP_NAMES = {
    0: "an associated const or type",
    1: "an associated function",
    2: "a `with_*` method",
    3: "a `&self` method",
    4: "a `&mut self` method",
    5: "a method taking `self` by value",
}


def member_key(member, owner):
    """(sort key, description) of one impl member, or None when not ranked."""
    if member.kind in ("const", "type"):
        return (0, 0, 0), GROUP_NAMES[0]
    if member.kind != "fn" or member.name is None:
        return None
    params, tail = parameters(member.header)
    receiver = receiver_group(params) if params is not None else None
    private = 0 if PUBLIC.match(member.head) else 1
    prefix = "private " if private else ""
    if member.name.startswith("with_"):
        return (2, 0, private), prefix + GROUP_NAMES[2]
    if receiver is not None:
        return (receiver, 0, private), prefix + GROUP_NAMES[receiver]
    returns_self = re.search(r"\bSelf\b", tail) or (
        owner is not None and re.search(r"\b" + re.escape(owner) + r"\b", tail.split("where")[0])
    )
    description = "a constructor" if returns_self else GROUP_NAMES[1]
    return (1, 0 if returns_self else 1, private), prefix + description


def check_member_order(path, lines, item, depth, report):
    """R22 over one inherent impl."""
    rows = body_rows(lines, item, depth)
    if rows is None:
        return
    owner = impl_target(item.header)[1]
    previous = None
    for member in parse_items(lines, rows[0], rows[1], depth + 1):
        ranked = member_key(member, owner)
        if ranked is None:
            continue
        key, description = ranked
        if previous is not None and key < previous[0]:
            report(path, member.head_row + 1, "R22",
                   f"{description} comes after {previous[1]}; the order is "
                   "associated consts and types, constructors, other associated functions, "
                   "`with_*` methods, `&self`, `&mut self`, then `self` by value, "
                   "`pub` before private in each group")
        else:
            previous = (key, description)


def check_item_order(path, lines, begin, end, depth, report):
    items = parse_items(lines, begin, end, depth)
    check_adjacency(path, lines, items, report)
    for item in items:
        if item.kind == "impl" and impl_target(item.header)[0] is None:
            check_member_order(path, lines, item, depth, report)
        elif item.kind == "mod":
            rows = body_rows(lines, item, depth)
            if rows is not None:
                check_item_order(path, lines, rows[0], rows[1], depth + 1, report)
