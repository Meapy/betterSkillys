"""Builds what the Elder dungeons need beyond the Doomed Realms port: their darker look and the
Elder monsters Doomed Realms never made. tools/port_doomed.py runs it first, so

    python tools/port_doomed.py --dr "<Doomed Realms repo>"

regenerates everything. Each Elder dungeon in DUNGEONS is built from betterSkillys' own map of the
regular dungeon (port_doomed_worlds.py does that, from this file's names):

- Tiles: every ground and wall the regular map uses gets an "Elder <id>" copy in a darker shade
  (the floor dark stone, the walls aged gold), drawn on the client's ElderAssets8x8.png. Each
  walkable ground also gets "Elder <id> Lit 1" to "Lit <LIGHT_LEVELS>", the floor around a brazier.
- Monsters: every monster the regular map places, or the regular dungeon's behaviours define,
  that has no Elder version in Doomed Realms gets an "Elder <id>" copy with the Elder stats
  (port_doomed.rebalance_elders). They share the regular monster's sprite and behaviour.

Outputs (generated, don't edit): source/Shared/resources/xml/custom/ElderGrounds.xml and
ElderObjects.xml, their client copies, and the sheet. Type codes are kept stable in
tools/elder/types.json.
"""

import copy
import glob
import json
import os
import re
import struct
import xml.etree.ElementTree as ET
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
XML_DIR = os.path.join(ROOT, "source", "Shared", "resources", "xml")
MAPS = os.path.join(ROOT, "source", "Shared", "resources", "worlds")
BEHAVIOURS = os.path.join(ROOT, "source", "WorldServer", "logic", "db")
CLIENT = os.path.join(ROOT, "client", "src")
IMAGES = os.path.join(CLIENT, "kabam", "rotmg", "assets", "prod", "images")
SHEET = os.path.join(CLIENT, "kabam", "rotmg", "assets", "custom", "images", "ElderAssets8x8.png")
SHEET_NAME = "elderAssets8x8"
OUTPUTS = {
    "grounds": [os.path.join(XML_DIR, "custom", "ElderGrounds.xml"),
                os.path.join(CLIENT, "kabam", "rotmg", "assets", "custom", "xmls", "ElderGrounds.xml")],
    "objects": [os.path.join(XML_DIR, "custom", "ElderObjects.xml"),
                os.path.join(CLIENT, "kabam", "rotmg", "assets", "custom", "xmls", "ElderObjects.xml")],
}
TYPES = os.path.join(ROOT, "tools", "elder", "types.json")
GROUND_TYPES = (0xE800, 0xF000)
OBJECT_TYPES = (0xF000, 0xF800)

FLOOR = (0.4, 0.36, 0.42)       # red, green and blue multipliers
WALL = (0.75, 0.6, 0.42)
LIGHT_TOP = (0.58, 0.47, 0.42)  # the floor beside a brazier
LIGHT_LEVELS = 6
LIGHT = [tuple(f + (t - f) * n / LIGHT_LEVELS for f, t in zip(FLOOR, LIGHT_TOP)) for n in range(1, LIGHT_LEVELS + 1)]
WALL_CLASSES = ("Wall", "ConnectedWall", "CaveWall")

# Doomed Realms world class -> (regular betterSkillys map, regular behaviour file, the objects
# that end a branch of a random layout, or None for the regular map's own layout)
DUNGEONS = {
    "ElderTombMap": ("Dungeons/Tomb of the Ancients.jm", "BehaviorDb.Tomb.cs", ["Elder Inactive Sarcophagus"]),
}


def load(files):
    objects, grounds = {}, {}
    for f in sorted(files):
        text = re.sub(r"<\?xml[^>]*\?>", "", open(f, encoding="utf-8-sig", errors="replace").read())
        try:
            root = ET.fromstring("<r>" + text + "</r>")
        except ET.ParseError:
            continue
        for o in root.iter("Object"):
            if o.get("id"):
                objects[o.get("id")] = o
        for g in root.iter("Ground"):
            if g.get("id"):
                grounds[g.get("id")] = g
    return objects, grounds


def xml_files(include_elder=False):
    out = []
    for f in glob.glob(os.path.join(XML_DIR, "**", "*.xml"), recursive=True):
        if not include_elder and os.path.abspath(f) in {os.path.abspath(p) for ps in OUTPUTS.values() for p in ps}:
            continue
        out.append(f)
    return out


def map_ids(path):
    """The grounds and objects a .jm map uses."""
    j = json.load(open(os.path.join(MAPS, path), encoding="utf-8-sig"))
    grounds = {e["ground"] for e in j["dict"] if e.get("ground")}
    objects = {o["id"] for e in j["dict"] for o in e.get("objs") or []}
    return grounds, objects


def behaviour_ids(name):
    """The monsters a behaviour file defines or refers to by id."""
    text = open(os.path.join(BEHAVIOURS, name), encoding="utf-8-sig").read()
    return set(re.findall(r'"([^"\n]+)"', text))


def sheet_sizes():
    """{sprite sheet: sprite width} from the client's AssetLoader."""
    text = open(os.path.join(CLIENT, "com", "company", "assembleegameclient", "util", "AssetLoader.as"), encoding="utf-8").read()
    return {m.group(1): int(m.group(2)) for m in re.finditer(r'addImageSet\("([^"]+)",[^;]*?,\s*(\d+)\s*,\s*\d+\s*\)', text)}


# ------------------------------------------------------------------------------------------------
# PNG (8-bit RGB/RGBA/palette/grey)

def read_png(path):
    data = open(path, "rb").read()
    pos, chunks = 8, []
    while pos < len(data):
        n, t = struct.unpack(">I4s", data[pos:pos + 8])
        chunks.append((t, data[pos + 8:pos + 8 + n]))
        pos += 12 + n
    w, h, depth, ctype = struct.unpack(">IIBB", next(d for t, d in chunks if t == b"IHDR")[:10])
    if depth != 8:
        raise ValueError("%s: only 8-bit images are supported" % path)
    plte = next((d for t, d in chunks if t == b"PLTE"), b"")
    trns = next((d for t, d in chunks if t == b"tRNS"), b"")
    raw = zlib.decompress(b"".join(d for t, d in chunks if t == b"IDAT"))
    bpp = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[ctype]
    stride = w * bpp
    rows, prev, i = [], bytearray(stride), 0
    for _ in range(h):
        f, line = raw[i], bytearray(raw[i + 1:i + 1 + stride])
        i += 1 + stride
        for x in range(stride):
            a = line[x - bpp] if x >= bpp else 0
            b = prev[x]
            c = prev[x - bpp] if x >= bpp else 0
            if f == 1:
                line[x] = (line[x] + a) & 255
            elif f == 2:
                line[x] = (line[x] + b) & 255
            elif f == 3:
                line[x] = (line[x] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(line)
        prev = line

    def px(x, y):
        r = rows[y]
        if ctype == 6:
            return tuple(r[x * 4:x * 4 + 4])
        if ctype == 2:
            return tuple(r[x * 3:x * 3 + 3]) + (255,)
        if ctype == 3:
            k = r[x]
            return tuple(plte[k * 3:k * 3 + 3]) + ((trns[k] if k < len(trns) else 255),)
        if ctype == 0:
            return (r[x],) * 3 + (255,)
        return (r[x * 2],) * 3 + (r[x * 2 + 1],)
    return w, h, px


def write_png(path, w, h, pix):
    raw = b"".join(b"\x00" + bytes(c for x in range(w) for c in pix[y][x]) for y in range(h))

    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
                + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


class Sheet:
    """The Elder sprite sheet: shaded 8x8 copies of betterSkillys sprites, 16 to a row."""

    def __init__(self, sizes):
        self.sizes, self.sprites, self.index = sizes, [], {}

    def add(self, sheet, index, shade):
        key = (sheet, index, shade)
        if key not in self.index:
            self.index[key] = len(self.sprites)
            self.sprites.append(key)
        return self.index[key]

    def retexture(self, element, shade):
        """Points every 8x8 texture of the element at a shaded copy on this sheet."""
        for node in element.iter():
            f, i = node.find("File"), node.find("Index")
            if f is None or i is None or self.sizes.get(f.text) != 8:
                continue
            i.text = "0x%02x" % self.add(f.text, int(i.text, 0), shade)
            f.text = SHEET_NAME

    def write(self):
        sources = {}
        rows = max(1, (len(self.sprites) + 15) // 16)
        w, h = 128, max(128, rows * 8)
        pix = [[(0, 0, 0, 0)] * w for _ in range(h)]
        for n, (sheet, index, shade) in enumerate(self.sprites):
            if sheet not in sources:
                sources[sheet] = read_png(os.path.join(IMAGES, sheet + ".png"))
            sw, _, px = sources[sheet]
            sx, sy = (index % (sw // 8)) * 8, (index // (sw // 8)) * 8
            dx, dy = (n % 16) * 8, (n // 16) * 8
            for y in range(8):
                for x in range(8):
                    r, g, b, a = px(sx + x, sy + y)
                    pix[dy + y][dx + x] = (int(r * shade[0]), int(g * shade[1]), int(b * shade[2]), a)
        write_png(SHEET, w, h, pix)


# ------------------------------------------------------------------------------------------------

class Types:
    """Type codes kept stable between runs."""

    def __init__(self):
        self.types = json.load(open(TYPES)) if os.path.exists(TYPES) else {}

    def get(self, key, span):
        if key not in self.types:
            used = {int(v, 16) for v in self.types.values()}
            self.types[key] = "0x%04X" % next(t for t in range(*span) if t not in used)
        return self.types[key]

    def save(self):
        os.makedirs(os.path.dirname(TYPES), exist_ok=True)
        with open(TYPES, "w") as f:
            json.dump(self.types, f, indent=1, sort_keys=True)


def write_xml(paths, root_tag, elements, comment):
    root = ET.Element(root_tag)
    for e in elements:
        root.append(e)
    ET.indent(root, "\t")
    text = "<!-- %s -->\n%s\n" % (comment, ET.tostring(root, encoding="unicode"))
    for p in paths:
        with open(p, "w", encoding="utf-8", newline="\r\n") as f:
            f.write(text)


def build(dr_ids):
    """Writes the Elder tiles and monsters; dr_ids are the Doomed Realms object ids."""
    import port_doomed  # for rebalance_elders
    objects, grounds = load(xml_files())
    sheet = Sheet(sheet_sizes())
    types = Types()
    new_grounds, new_objects = [], []
    made_grounds, made_objects = set(), set()

    def elder_ground(gid, shade, name):
        if name in made_grounds or gid not in grounds:
            return
        g = copy.deepcopy(grounds[gid])
        g.set("id", name)
        g.set("type", types.get("ground:" + name, GROUND_TYPES))
        sheet.retexture(g, shade)
        new_grounds.append(g)
        made_grounds.add(name)

    for cls, (path, behaviours, _) in sorted(DUNGEONS.items()):
        map_grounds, map_objects = map_ids(path)
        for gid in sorted(map_grounds):
            if gid == "Empty" or gid.startswith("Elder "):
                continue
            elder_ground(gid, FLOOR, "Elder " + gid)
            if gid in grounds and grounds[gid].find("NoWalk") is None:
                for n, shade in enumerate(LIGHT, 1):
                    elder_ground(gid, shade, "Elder %s Lit %d" % (gid, n))
        monsters = set()
        for oid in sorted(map_objects | behaviour_ids(behaviours)):
            o = objects.get(oid)
            if o is None or oid.startswith("Elder "):
                continue
            name = "Elder " + oid
            if o.findtext("Class") in WALL_CLASSES and oid in map_objects:
                if name in made_objects or name in objects:
                    continue
                w = copy.deepcopy(o)
                w.set("id", name)
                w.set("type", types.get(name, OBJECT_TYPES))
                sheet.retexture(w, WALL)
                new_objects.append(w)
                made_objects.add(name)
            elif o.find("Enemy") is not None and name not in dr_ids and name not in objects and name not in made_objects:
                monsters.add(oid)
        copies = {}
        for oid in sorted(monsters):
            m = copy.deepcopy(objects[oid])
            m.set("id", "Elder " + oid)
            m.set("type", types.get("Elder " + oid, OBJECT_TYPES))
            display = m.find("DisplayId")
            if display is None:
                display = ET.SubElement(m, "DisplayId")
                display.text = oid
            display.text = "Elder " + display.text
            copies["Elder " + oid] = m
        port_doomed.rebalance_elders(copies, objects)
        for name, m in sorted(copies.items()):
            new_objects.append(m)
            made_objects.add(name)

    write_xml(OUTPUTS["grounds"], "GroundTypes", new_grounds,
              "Generated by tools/make_elder.py: the Elder dungeons' darker grounds. Do not edit.")
    write_xml(OUTPUTS["objects"], "Objects", new_objects,
              "Generated by tools/make_elder.py: the Elder dungeons' walls and the Elder monsters Doomed Realms didn't make. Do not edit.")
    sheet.write()
    types.save()
    return len(new_grounds), len(new_objects), len(sheet.sprites)


def lit_grounds():
    """The Elder grounds that have lit versions."""
    _, grounds = load(OUTPUTS["grounds"][:1])
    return {gid for gid in grounds if gid.startswith("Elder ") and gid + " Lit 1" in grounds}


if __name__ == "__main__":
    import sys
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    dr = load(glob.glob(os.path.join(XML_DIR, "custom", "DoomedRealms.xml")))[0]
    print("Wrote %d Elder grounds, %d Elder objects, %d sprites." % build(set(dr)))
