"""Dungeons for tools/port_doomed.py.

Reads the Doomed Realms world classes (wserver/realm/worlds/*.cs) and their .wmap maps, converts
the maps to betterSkillys .jm files in source/Shared/resources/worlds/Doomed/, and writes the world
definitions (source/Shared/resources/xml/custom/DoomedRealmsWorlds.xml) that portals point at
through <DungeonName>.

.wmap (Doomed Realms wserver/realm/terrain/Wmap.cs): a version byte, then a zlib stream of
  int16 dictionary size; per entry: int16 ground type, string object id, string object name,
  byte terrain, byte region (v1: + byte elevation); int32 width, int32 height;
  per tile, row by row: int16 dictionary index (v2: + byte elevation).
  Strings are .NET BinaryReader strings (7-bit encoded length, UTF-8), little-endian numbers.
.jm (betterSkillys WorldServer/core/terrain/Json2Wmap.cs): JSON with width, height,
  dict [{ground, objs [{id, name}], regions [{id}]}] and data, the base64 of a zlib stream of
  big-endian int16 dictionary indexes.
"""

import base64
import glob
import json
import os
import re
import struct
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WORLDS_DIR = os.path.join(ROOT, "source", "Shared", "resources", "worlds", "Doomed")
WORLDS_XML = os.path.join(ROOT, "source", "Shared", "resources", "xml", "custom", "DoomedRealmsWorlds.xml")

# Doomed Realms' TileRegion (wserver/realm/JsonMap.cs), in order
REGIONS = ["None", "Spawn", "Realm_Portals", "Store_1", "Store_2", "Store_3", "Store_4", "Store_5", "Store_6",
           "Store_7", "Store_8", "Store_9", "Vault", "Loot", "Defender", "Hallway", "Hallway_1",
           "Hallway_2", "Hallway_3", "Enemy"]

# Doomed Realms worlds ported as dungeons: world class -> portal ids that lead there.
# (The classic dungeons, nexus, vault, guild halls, market and tutorial are betterSkillys' own.)
PORTED_WORLDS = {
    "ElderPirateCave": ["Elder Pirate Cave Portal"],
    "ElderDraconis": ["Elder Draconis Portal"],
    "ElderAbyss": ["Elder Abyss Portal"],
    "ElderSnakePit": ["Elder Snake Pit Portal"],
    "ElderSpiderDen": ["Elder Spider Den Portal"],
    "ElderSpriteWorld": ["Elder Sprite World Portal"],
    "ElderTombMap": ["Elder Tomb Portal"],
    "ElderUndeadL": ["Elder Undead Lair Portal"],
    "Dcellar": ["Doomed Cellar Portal"],
    "Dragonlord": ["Hash Cellar Portal"],
    "DoomCrystal": ["Doom Crystal Portal"],
    "Island": ["Island Portal"],
    "ZombieMap": ["Zombies Portal"],
    "ZombieMG": ["Zombie Minigame Portal"],
    "Herding": ["Sheep Herding Portal"],
    "ForestMap": ["Forest Sanctuary Portal"],
    "ChristmasOryxsChamber": ["Oryx's Christmas Portal"],
    "PartyCellarMap": ["Birthday Portal"],
    "Kitchen": ["Kitchen Portal"],
    "Secret": ["Secret Portal"],
    "BattleArenaMap": ["Battle Arena Portal"],
    "BattleArenaMap2": ["Free Battle Arena Portal"],
    "Mine": [],
    "GauntletMap": [],
    "TurkeyMap": [],
    "Admin": [],
    "ShopMap": [],
}

# Elder dungeons Doomed Realms left unfinished (it made their keys, portals and monsters, but no
# maps): built like its finished ones, from the regular dungeon's maps with every object that has
# an "Elder <id>" version swapped for it. world -> (regular world class, name, portal ids)
ELDER_WORLDS = {
    "ElderManor": ("Manor", "Elder Manor", ["Elder Manor Portal"]),
    "ElderOceanTrench": ("OceanTrench", "Elder Ocean Trench", ["Elder Ocean Trench Portal"]),
    "ElderJungle": ("Jungle", "Elder Jungle", ["Elder Jungle Portal"]),
    "ElderBeachZone": ("BeachZone", "Elder Beach Zone", ["Elder Beach Portal"]),
    "ElderDavy": ("Davy", "Elder Davy Jones' Locker", ["Elder Davy Portal"]),
}
# Doomed Realms' shop map places merchants as objects; betterSkillys places them on store regions
SHOP_WORLD = "ShopMap"


class Reader:
    def __init__(self, data):
        self.data, self.pos = data, 0

    def take(self, fmt):
        v = struct.unpack_from("<" + fmt, self.data, self.pos)
        self.pos += struct.calcsize("<" + fmt)
        return v[0]

    def string(self):
        n, shift = 0, 0
        while True:
            b = self.data[self.pos]
            self.pos += 1
            n |= (b & 0x7F) << shift
            if not b & 0x80:
                break
            shift += 7
        s = self.data[self.pos:self.pos + n].decode("utf-8")
        self.pos += n
        return s


def read_wmap(path):
    raw = open(path, "rb").read()
    ver = raw[0]
    r = Reader(zlib.decompress(raw[1:]))
    entries = []
    for _ in range(r.take("h")):
        ground = r.take("h")
        obj = r.string()
        name = r.string()
        r.take("B")  # terrain
        region = r.take("B")
        if ver == 1:
            r.take("B")
        entries.append((ground & 0xFFFF, obj, name, region))
    w, h = r.take("i"), r.take("i")
    grid = []
    for _ in range(w * h):
        grid.append(r.take("h"))
        if ver == 2:
            r.take("B")
    return {"width": w, "height": h, "dict": entries, "grid": grid}


def parse_world_classes(dr_root):
    """{class: (display name, background, allow teleport, [wmap file names])}."""
    worlds = {}
    folder = os.path.join(dr_root, "wserver", "realm", "worlds")
    for f in glob.glob(os.path.join(folder, "*.cs")):
        t = open(f, encoding="utf-8-sig", errors="replace").read()
        cls = re.search(r"class (\w+)\s*:\s*World", t)
        if not cls:
            continue
        name = re.search(r'Name\s*=\s*"([^"]+)"', t)
        bg = re.search(r"Background\s*=\s*(\d+)", t)
        tp = re.search(r"AllowTeleport\s*=\s*(true|false)", t)
        maps = []
        for m in re.finditer(r'"wServer\.realm\.worlds\.([^"]+?)"(\s*\+\s*r\.Next\((\d+),\s*(\d+)\s*(\+\s*1)?\))?', t):
            if m.group(2):
                hi = int(m.group(4)) + (1 if m.group(5) else 0)
                maps += ["%s%d.wmap" % (m.group(1), i) for i in range(int(m.group(3)), hi)]
            else:
                maps.append(m.group(1))
        worlds[cls.group(1)] = (name.group(1) if name else cls.group(1), int(bg.group(1)) if bg else 0,
                                tp is None or tp.group(1) == "true", maps)
    return worlds


def find_map(dr_root, name):
    folder = os.path.join(dr_root, "wserver", "realm", "worlds")
    for f in os.listdir(folder):
        if f.lower() == name.lower():
            return os.path.join(folder, f)
    return None


def ground_names(xml_files):
    """{ground type: id} from <Ground> elements."""
    import xml.etree.ElementTree as ET
    out = {}
    for f in xml_files:
        text = re.sub(r"<\?xml[^>]*\?>", "", open(f, encoding="utf-8", errors="replace").read())
        try:
            root = ET.fromstring("<r>" + text + "</r>")
        except ET.ParseError:
            continue
        for g in root.iter("Ground"):
            if g.get("type") and g.get("id"):
                out[int(g.get("type"), 16)] = g.get("id")
    return out


def to_jm(wmap, dr_grounds, object_id):
    """betterSkillys .jm text for a read .wmap; object_id maps a Doomed Realms object id to the ported one."""
    entries = []
    for ground, obj, name, region in wmap["dict"]:
        e = {}
        if ground not in (0xFF, 0xFFFF) and ground in dr_grounds:
            e["ground"] = dr_grounds[ground]
        if obj:
            o = {"id": object_id(obj)}
            if name:
                o["name"] = name
            e["objs"] = [o]
        if 0 < region < len(REGIONS):
            e["regions"] = [{"id": REGIONS[region]}]
        entries.append(e)
    data = b"".join(struct.pack(">h", i) for i in wmap["grid"])
    return json.dumps({"width": wmap["width"], "height": wmap["height"], "dict": entries,
                       "data": base64.b64encode(zlib.compress(data, 9)).decode("ascii")})


def elder_swap(wmap, dr_ids):
    """The map with each object id X replaced by "Elder X" where Doomed Realms defines one."""
    entries = [(g, "Elder " + o if o and "Elder " + o in dr_ids else o, n, r) for (g, o, n, r) in wmap["dict"]]
    return dict(wmap, dict=entries)


def shop_regions(wmap):
    """Merchant objects become Store_1 region tiles (where the shop world puts its merchants)."""
    store = REGIONS.index("Store_1")
    entries = [(g, "", "", store) if o == "Merchant" else (g, o, n, r) for (g, o, n, r) in wmap["dict"]]
    return dict(wmap, dict=entries)


def world_maps(dr_root, dr_ids=frozenset()):
    """{class: (world name, background, allow teleport, [read wmaps])} for PORTED_WORLDS and ELDER_WORLDS."""
    classes = parse_world_classes(dr_root)
    out = {}
    for cls in PORTED_WORLDS:
        name, bg, tp, maps = classes[cls]
        maps = [read_wmap(find_map(dr_root, m)) for m in maps]
        if cls == SHOP_WORLD:
            maps = [shop_regions(m) for m in maps]
        out[cls] = (name, bg, tp, maps)
    for cls, (base, name, _) in ELDER_WORLDS.items():
        _, bg, tp, maps = classes[base]
        out[cls] = (name, bg, tp, [elder_swap(read_wmap(find_map(dr_root, m)), dr_ids) for m in maps])
    return out


def portal_worlds():
    """{portal id: world class} for the ported worlds."""
    out = {pid: cls for cls, pids in PORTED_WORLDS.items() for pid in pids}
    out.update({pid: cls for cls, (_, _, pids) in ELDER_WORLDS.items() for pid in pids})
    return out


def write_worlds(worlds, dr_grounds, object_id):
    """Writes the .jm maps and DoomedRealmsWorlds.xml; returns {class: world id}."""
    if os.path.isdir(WORLDS_DIR):
        for f in glob.glob(os.path.join(WORLDS_DIR, "*.jm")):
            os.remove(f)
    os.makedirs(WORLDS_DIR, exist_ok=True)
    ids, lines = {}, []
    for cls, (name, bg, tp, maps) in sorted(worlds.items()):
        safe = re.sub(r"[^A-Za-z0-9 '\-]", "", name).strip() or cls
        files = []
        for i, wm in enumerate(maps):
            fname = safe + (" %d" % (i + 1) if len(maps) > 1 else "") + ".jm"
            with open(os.path.join(WORLDS_DIR, fname), "w", encoding="utf-8") as f:
                f.write(to_jm(wm, dr_grounds, object_id))
            files.append("Doomed/" + fname)
        ids[cls] = name
        w = max(m["width"] for m in maps)
        h = max(m["height"] for m in maps)
        lines.append('\t<World id="%s">' % name.replace("&", "&amp;").replace('"', "&quot;"))
        lines.append("\t\t<Width>%d</Width>" % w)
        lines.append("\t\t<Height>%d</Height>" % h)
        lines += ["\t\t<MapJM>%s</MapJM>" % f.replace("&", "&amp;") for f in files]
        lines.append("\t\t<VisibilityType>1</VisibilityType>")
        if not tp:
            lines.append("\t\t<DisableTeleport/>")
        lines.append("\t\t<Dungeon/>")
        lines.append("\t</World>")
    with open(WORLDS_XML, "w", encoding="utf-8", newline="\r\n") as f:
        f.write("<!-- Generated by tools/port_doomed.py (port_doomed_worlds.py) from the Doomed Realms worlds. Do not edit. -->\n")
        f.write("<Worlds>\n" + "\n".join(lines) + "\n</Worlds>\n")
    return ids
