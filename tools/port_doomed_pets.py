"""Pet behaviours for tools/port_doomed.py.

Reads the Doomed Realms pet definitions (wserver/logic/db/BehaviorDb.Pets.cs and HealPets.cs) and
writes source/WorldServer/logic/db/BehaviorDb.DoomedPets.cs for the ported pets:
  * PetFollow (betterSkillys' follow, which also teleports the pet back when it is 20+ tiles away)
  * PetHeal for PetsHealingHP / PetsHealingMP, with the same amounts and cooldowns
  * PetStrike for PetAttack and ThrowAttackPet, which hit the nearest enemy directly
  * PetShoot for the projectile attacks (PetSimpleAttack, PetMultiAttack, PetRingAttack,
    PetPredictiveMultiAttack), with the same projectile, count, spread and offset
betterSkillys' own permanent pets, which had no behaviour, get the follow.
"""

import glob
import math
import os
import re
from xml.sax import saxutils

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PET_BEHAVIOR_FILES = ["BehaviorDb.Pets.cs", "BehaviorDb.HealPets.cs"]
PET_BEHAVIORS_CS = os.path.join(ROOT, "source", "WorldServer", "logic", "db", "BehaviorDb.DoomedPets.cs")
ITEM_XMLS = [os.path.join(ROOT, "source", "Shared", "resources", "xml", "custom", "DoomedRealms.xml"),
             os.path.join(ROOT, "client", "src", "kabam", "rotmg", "assets", "custom", "xmls", "DoomedRealms.xml")]
# shots this close to the aim count towards a pet's damage on one enemy
HIT_CONE = 15.0


def balanced(text, start):
    """Index just past the parenthesis that closes the one at text[start]."""
    depth = 0
    for i in range(start, len(text)):
        if text[i] == "(":
            depth += 1
        elif text[i] == ")":
            depth -= 1
            if depth == 0:
                return i + 1
    return len(text)


def projectile_damage(obj, index):
    for p in obj.iter("Projectile"):
        if int(p.get("id", "0")) == index:
            dmg = p.findtext("Damage")
            if dmg:
                return int(dmg), int(dmg)
            lo, hi = p.findtext("MinDamage"), p.findtext("MaxDamage")
            if lo and hi:
                return int(lo), int(hi)
    return None


def parse_attack(call):
    a = re.match(r"new PetAttack\((\d+),\s*(\d+),\s*([\d.]+)\)", call)
    if a:
        return ("direct", int(a.group(1)), int(a.group(2)), float(a.group(3)))
    a = re.match(r"ThrowAttackPet\.Instance\(([\d.]+),\s*([\d.]+),\s*(\d+),\s*(\d+)\)", call)
    if a:
        return ("direct", int(a.group(3)), int(a.group(4)), float(a.group(2)))
    a = re.match(r"(Pet(?:Simple|Multi|Ring|PredictiveMulti)Attack)\.Instance\((.*)\)$", call, re.S)
    if not a:
        return None
    kind = a.group(1)
    # Doomed Realms' parameters. Angles are radians, mostly written "<degrees>*(float)Math.PI/180".
    names = {
        "PetSimpleAttack": ["radius", "projectileIndex"],
        "PetMultiAttack": ["radius", "angle", "numShot", "offset", "projectileIndex"],
        "PetPredictiveMultiAttack": ["radius", "angle", "numShot", "predictive", "projectileIndex"],
        "PetRingAttack": ["count", "radius", "offset", "projectileIndex"],
    }[kind]
    args = {}
    for n, arg in enumerate(x.strip() for x in a.group(2).split(",")):
        named = re.match(r"(\w+)\s*:\s*(.*)$", arg, re.S)
        if named:
            args[named.group(1)] = named.group(2)
        elif n < len(names):
            args[names[n]] = arg

    def number(key, default):
        m = re.match(r"\s*(-?[\d.]+)", args.get(key, ""))
        return float(m.group(1)) if m else default

    def degrees(key):
        if "PI" in args.get(key, ""):
            return number(key, 0.0)
        return math.degrees(number(key, 0.0))

    index = int(number("projectileIndex", 0))
    if kind == "PetRingAttack":
        # a ring starts at the target; PetShoot centres its spread on it
        count = max(1, int(number("count", 1)))
        step = 360.0 / count
        return ("projectile", index, count, number("radius", 10.0), step, degrees("offset") + step * (count - 1) / 2)
    count = max(1, int(number("numShot", 1)))
    offset = degrees("offset") if kind == "PetMultiAttack" else 0.0
    return ("projectile", index, count, number("radius", 10.0), degrees("angle"), offset)


def parse_pet_behaviors(dr_root):
    """Doomed Realms pet definitions: list of ((type min, type max), definition)."""
    defs = []
    for name in PET_BEHAVIOR_FILES:
        path = os.path.join(dr_root, "wserver", "logic", "db", name)
        text = re.sub(r"/\*.*?\*/", "", open(path, encoding="utf-8-sig").read(), flags=re.S)
        text = re.sub(r"//[^\n]*", "", text)
        starts = [m.start() for m in re.finditer(r"\.Init(?:Many)?\(", text)]
        for n, s in enumerate(starts):
            block = text[s:starts[n + 1] if n + 1 < len(starts) else len(text)]
            many = re.match(r"\.InitMany\((0x[0-9a-fA-F]+),\s*(0x[0-9a-fA-F]+)", block)
            one = re.match(r"\.Init\((0x[0-9a-fA-F]+)", block)
            if not many and not one:
                continue
            lo = int((many or one).group(1), 16)
            hi = int(many.group(2), 16) if many else lo
            d = {"hp": None, "mp": None, "attacks": [], "many": bool(many)}
            for c in re.finditer(r"Cooldown\.Instance\(", block):
                inner = block[c.end():balanced(block, c.end() - 1) - 1]
                m = re.match(r"\s*(\d+)\s*,\s*(.*)$", inner, re.S)
                if not m:
                    continue
                ms, call = int(m.group(1)), m.group(2).strip()
                h = re.match(r"new PetsHealing(HP|MP)\((\d+),\s*(\d+)\)", call)
                if h:
                    d["hp" if h.group(1) == "HP" else "mp"] = (ms, int(h.group(2)), int(h.group(3)))
                    continue
                attack = parse_attack(call)
                if attack:
                    d["attacks"].append((ms,) + attack)
            defs.append(((lo, hi), d))
    return defs


def cs_string(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def write_pet_behaviors(dr_root, dr, chosen, bs, types):
    given = {a.get("objectId") for o in chosen.values() for a in o.iter("Activate") if a.text == "PermaPet"}
    pets = {oid for oid, o in chosen.items() if o.findtext("Class") == "Character" and oid in given}
    by_dr_type = {int(chosen[oid].get("type"), 16): oid for oid in pets}
    behaviors = {}
    # ranges (InitMany) first, single definitions override them
    for (lo, hi), d in sorted(parse_pet_behaviors(dr_root), key=lambda x: not x[1]["many"]):
        for t in range(lo, hi + 1):
            if t in by_dr_type:
                behaviors[by_dr_type[t]] = d
    existing = set()
    for f in glob.glob(os.path.join(ROOT, "source", "WorldServer", "logic", "db", "*.cs")):
        if os.path.abspath(f) != os.path.abspath(PET_BEHAVIORS_CS):
            existing |= set(re.findall(r'\.Init\("([^"]+)"', open(f, encoding="utf-8-sig", errors="replace").read()))
    bs_pets = {a.get("objectId") for o in bs.values() for a in o.iter("Activate")
               if a.text == "PermaPet" and a.get("objectId") in bs} - existing

    lines, stats = [], {}
    for oid in sorted(pets, key=lambda p: types[p]):
        d = behaviors.get(oid, {"hp": None, "mp": None, "attacks": []})
        parts = ["new PetFollow()"]
        stat = stats[oid] = {}
        for key, mana in (("hp", "false"), ("mp", "true")):
            if d[key]:
                ms, lo, hi = d[key]
                parts.append("new PetHeal(%s, %d, %d, %d)" % (mana, lo, hi, ms))
                stat[key] = (lo, hi, ms)
        dps = 0.0
        for attack in d["attacks"]:
            ms, kind = attack[0], attack[1]
            if kind == "direct":
                lo, hi, rng = attack[2], attack[3], attack[4]
                parts.append("new PetStrike(%d, %d, %d, %g)" % (lo, hi, ms, rng))
                dps += (lo + hi) / 2 * 1000 / ms
            elif projectile_damage(chosen[oid], attack[2]):
                index, count, rng, angle, offset = attack[2:]
                parts.append("new PetShoot(%g, %d, %g, %g, %d, %d)" % (
                    rng, count, round(angle, 2), round(offset % 360, 2), index, ms))
                lo, hi = projectile_damage(chosen[oid], index)
                dps += (lo + hi) / 2 * shots_on_target(count, angle, offset) * 1000 / ms
        if dps:
            stat["dps"] = round(dps)
        lines.append("            .Init(%s, new State(%s))" % (cs_string(oid), ", ".join(parts)))
    for oid in sorted(bs_pets):
        lines.append("            .Init(%s, new State(new PetFollow()))" % cs_string(oid))

    with open(PET_BEHAVIORS_CS, "w", encoding="utf-8", newline="\r\n") as f:
        f.write("// Generated by tools/port_doomed.py (port_doomed_pets.py). Do not edit.\n")
        f.write("// Permanent pets: the Doomed Realms pets with their healing and attacks (PetHeal, PetStrike, PetShoot),\n")
        f.write("// and betterSkillys' own permanent pets, which had no behaviour, with a follow.\n")
        f.write("using WorldServer.logic.behaviors;\nusing WorldServer.logic.behaviors.doomed;\n\n")
        f.write("namespace WorldServer.logic\n{\n    partial class BehaviorDb\n    {\n")
        f.write("        private _ DoomedPets = () => Behav()\n")
        f.write("\n".join(lines) + ";\n")
        f.write("    }\n}\n")
    annotate_generators(stats)
    return len(pets), len(bs_pets)


def shots_on_target(count, angle, offset):
    """How many shots of a PetShoot volley fly at the enemy it aims at."""
    start = -angle * (count - 1) / 2 + offset
    return sum(1 for i in range(count) if abs((start + angle * i + 180) % 360 - 180) <= HIT_CONE)


def annotate_generators(stats):
    """Puts each pet's damage per second (on one enemy) and healing on the PermaPet activates of
    the items that give it, for the item tooltips: petDps, petHp / petMp ("min-max") and
    petHpMs / petMpMs (every how many milliseconds)."""
    for path in ITEM_XMLS:
        text = open(path, encoding="utf-8", newline="").read()

        def annotate(m):
            stat = stats.get(saxutils.unescape(m.group(1), {"&quot;": '"', "&apos;": "'"}), {})
            attrs = re.sub(r'\s+pet\w+="[^"]*"', "", m.group(2))
            if "dps" in stat:
                attrs += ' petDps="%d"' % stat["dps"]
            for key, name in (("hp", "Hp"), ("mp", "Mp")):
                if key in stat:
                    lo, hi, ms = stat[key]
                    attrs += ' pet%s="%s" pet%sMs="%d"' % (name, lo if lo == hi else "%d-%d" % (lo, hi), name, ms)
            return '<Activate objectId="%s"%s>PermaPet</Activate>' % (m.group(1), attrs)

        text = re.sub(r'<Activate objectId="([^"]*)"([^>]*)>PermaPet</Activate>', annotate, text)
        with open(path, "w", encoding="utf-8", newline="") as f:
            f.write(text)
