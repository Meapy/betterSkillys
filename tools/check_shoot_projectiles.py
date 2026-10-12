"""Lists Shoot behaviours whose projectile index the monster's XML doesn't define.

Shoot.cs logs "<name> Doesnt have projectile: N" and skips such shots, so the attack never
happens. Each `new Shoot(...)` in logic/db is attributed to the `.Init("name", ...)` call that
contains it; a Shoot inside a helper method (`static State Foo() => ...`) is checked against every
Init that calls the helper. Run from the repository root:

    python tools/check_shoot_projectiles.py [-v]

Exits 1 if any mismatch is found.
"""
import glob
import os
import re
import sys
import xml.etree.ElementTree as ET

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
XML_DIR = os.path.join(ROOT, "source", "Shared", "resources", "xml")
DB_DIR = os.path.join(ROOT, "source", "WorldServer", "logic", "db")
SHOOT_PARAMS = ["radius", "count", "shootAngle", "projectileIndex"]


def load_projectiles():
    """Object id -> set of projectile ids (the last definition of an id wins, as in XmlData)."""
    objects = {}
    for path in sorted(glob.glob(os.path.join(XML_DIR, "**", "*.xml"), recursive=True)):
        try:
            tree = ET.parse(path)
        except ET.ParseError as e:
            print(f"warning: can't parse {path}: {e}", file=sys.stderr)
            continue
        for obj in tree.iter("Object"):
            oid = obj.get("id")
            if oid is None:
                continue
            ids = set()
            for p in obj.findall("Projectile"):
                ids.add(int(p.get("id", "0"), 0))
            objects[oid] = ids
    return objects


def strip_comments(src):
    """Blanks out comments and string contents' comment markers while keeping offsets."""
    out = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if c == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            out.append(src[i:j + 1])
            i = j + 1
        elif src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            out.append(" " * (j - i))
            i = j
        elif src.startswith("/*", i):
            j = src.find("*/", i + 2)
            j = n if j < 0 else j + 2
            out.append(re.sub(r"[^\n]", " ", src[i:j]))
            i = j
        else:
            out.append(c)
            i += 1
    return "".join(out)


def match_paren(src, open_idx):
    """Index of the parenthesis closing the one at open_idx (strings skipped)."""
    depth = 0
    i = open_idx
    while i < len(src):
        c = src[i]
        if c == '"':
            i += 1
            while src[i] != '"':
                i += 2 if src[i] == "\\" else 1
        elif c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    raise ValueError("unbalanced parentheses")


def split_args(s):
    args, depth, cur, i = [], 0, [], 0
    while i < len(s):
        c = s[i]
        if c == '"':
            j = i + 1
            while s[j] != '"':
                j += 2 if s[j] == "\\" else 1
            cur.append(s[i:j + 1])
            i = j + 1
            continue
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        if c == "," and depth == 0:
            args.append("".join(cur).strip())
            cur = []
        else:
            cur.append(c)
        i += 1
    if "".join(cur).strip():
        args.append("".join(cur).strip())
    return args


def projectile_index(args):
    for a in args:
        m = re.match(r"projectileIndex\s*:\s*(.+)$", a, re.S)
        if m:
            return m.group(1).strip()
    positional = [a for a in args if not re.match(r"^[A-Za-z_]\w*\s*:(?!:)", a)]
    return positional[3] if len(positional) > 3 else "0"


def scan_file(path):
    with open(path, encoding="utf-8-sig") as f:
        raw = f.read()
    src = strip_comments(raw)
    line_of = lambda idx: raw.count("\n", 0, idx) + 1

    inits = []  # (start, end, name)
    for m in re.finditer(r'\.Init\(\s*"([^"]+)"', src):
        start = m.start() + len(".Init")
        inits.append((start, match_paren(src, start), m.group(1)))
    many = []
    for m in re.finditer(r'\.InitMany\(\s*"([^"]+)"\s*,\s*"([^"]+)"', src):
        start = m.start() + len(".InitMany")
        many.append((start, match_paren(src, start), f"{m.group(1)}..{m.group(2)}"))
    helpers = []  # (start, end, method name)
    for m in re.finditer(r"\b(?:private|internal|public)?\s*static\s+State\s+(\w+)\s*\(", src):
        body = src.find("=>", m.end())
        brace = src.find("{", m.end())
        if body < 0 or (0 <= brace < body):
            end = match_brace(src, brace)
        else:
            end = src.find(";", body)
        helpers.append((m.start(), end, m.group(1)))

    calls = []
    for m in re.finditer(r"\bnew\s+Shoot\s*\(", src):
        open_idx = m.end() - 1
        args = split_args(src[open_idx + 1:match_paren(src, open_idx)])
        idx = projectile_index(args)
        owners = [name for s, e, name in inits if s < m.start() < e]
        kind = "init"
        if not owners:
            owners = [name for s, e, name in many if s < m.start() < e]
            kind = "initmany" if owners else kind
        if not owners:
            hs = [name for s, e, name in helpers if s < m.start() < e]
            if hs:
                kind = "helper:" + hs[0]
                owners = None
        calls.append({"file": os.path.basename(path), "line": line_of(m.start()), "index": idx,
                      "owners": owners, "kind": kind, "src": src})
    return calls, inits


def match_brace(src, open_idx):
    depth = 0
    for i in range(open_idx, len(src)):
        if src[i] == "{":
            depth += 1
        elif src[i] == "}":
            depth -= 1
            if depth == 0:
                return i
    return len(src)


def main():
    verbose = "-v" in sys.argv
    projectiles = load_projectiles()
    all_calls, all_inits = [], []
    for path in sorted(glob.glob(os.path.join(DB_DIR, "*.cs"))):
        calls, inits = scan_file(path)
        all_calls += calls
        all_inits += [(path, s, e, name) for s, e, name in inits]

    sources = {}
    for path, s, e, name in all_inits:
        if path not in sources:
            with open(path, encoding="utf-8-sig") as f:
                sources[path] = strip_comments(f.read())

    bad, unknown = [], []
    for c in all_calls:
        owners = c["owners"]
        if c["kind"].startswith("helper:"):
            helper = c["kind"].split(":", 1)[1]
            owners = [name for path, s, e, name in all_inits
                      if re.search(r"\b" + helper + r"\s*\(", sources[path][s:e])]
        if c["kind"] == "initmany" or not owners:
            unknown.append(c)
            continue
        try:
            idx = int(c["index"], 0)
        except ValueError:
            unknown.append(c)
            continue
        for owner in owners:
            have = projectiles.get(owner)
            if have is None:
                continue  # no XML: BehaviorDb logs "Xml data not found" and skips it
            if idx not in have:
                bad.append((c["file"], c["line"], owner, idx, sorted(have)))

    by_owner = {}
    for f, line, owner, idx, have in bad:
        by_owner.setdefault((f, owner, tuple(have)), []).append((line, idx))
    for (f, owner, have), items in sorted(by_owner.items()):
        lines = ", ".join(f"{line}:{idx}" for line, idx in items)
        print(f"{f}  {owner!r}  has {list(have)}  ({len(items)} calls)  line:index {lines}")
    print(f"\n{len(bad)} mismatched Shoot calls out of {len(all_calls)}", file=sys.stderr)
    if verbose and unknown:
        print("\nNot checked (InitMany, no owning Init, or a computed index):", file=sys.stderr)
        for c in unknown:
            print(f"  {c['file']}:{c['line']} {c['kind']} index={c['index']}", file=sys.stderr)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
