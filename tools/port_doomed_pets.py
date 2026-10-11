"""Pet behaviours for tools/port_doomed.py.

Reads the Doomed Realms pet definitions (wserver/logic/db/BehaviorDb.Pets.cs and HealPets.cs) and
writes source/WorldServer/logic/db/BehaviorDb.DoomedPets.cs for the ported pets:
  * PetFollow (betterSkillys' follow, which also teleports the pet back when it is 20+ tiles away)
  * PetHeal for PetsHealingHP / PetsHealingMP, with the same amounts and cooldowns
  * PetStrike for the attacks. PetAttack and ThrowAttackPet keep their damage; pets that fired
    projectiles (PetSimpleAttack, PetMultiAttack, PetRingAttack, ...) hit the nearest enemy
    directly for that projectile's damage (multi-shots count up to 3 projectiles).
betterSkillys' own permanent pets, which had no behaviour, get the follow.
"""

import glob
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PET_BEHAVIOR_FILES = ["BehaviorDb.Pets.cs", "BehaviorDb.HealPets.cs"]
PET_BEHAVIORS_CS = os.path.join(ROOT, "source", "WorldServer", "logic", "db", "BehaviorDb.DoomedPets.cs")


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
    a = re.match(r"(Pet(?:Simple|Multi|Ring|Predictive|PredictiveMulti|PredictiveRing)Attack)\.Instance\((.*)\)$", call, re.S)
    if not a:
        return None
    kind, args = a.group(1), a.group(2)
    named = re.search(r"projectileIndex\s*:\s*(\d+)", args)
    positional = [x.strip() for x in re.sub(r"\w+\s*:\s*[^,]+", "", args).split(",") if x.strip()]
    index = int(named.group(1)) if named else 0
    count, radius = 1, 10.0
    try:
        if kind == "PetRingAttack":
            if not named and len(positional) >= 4:
                index = int(positional[3])
            radius = 8.0
        else:
            radius = float(positional[0])
            if kind == "PetSimpleAttack" and not named and len(positional) >= 2:
                index = int(positional[1])
            if "Multi" in kind and len(positional) >= 3:
                count = max(1, min(int(re.sub(r"[^\d]", "", positional[2]) or 1), 3))
    except ValueError:
        pass
    return ("projectile", index, count, radius)


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

    lines = []
    for oid in sorted(pets, key=lambda p: types[p]):
        d = behaviors.get(oid, {"hp": None, "mp": None, "attacks": []})
        parts = ["new PetFollow()"]
        for key, mana in (("hp", "false"), ("mp", "true")):
            if d[key]:
                ms, lo, hi = d[key]
                parts.append("new PetHeal(%s, %d, %d, %d)" % (mana, lo, hi, ms))
        for attack in d["attacks"]:
            ms, kind = attack[0], attack[1]
            if kind == "direct":
                lo, hi, rng = attack[2], attack[3], attack[4]
            else:
                index, count, rng = attack[2], attack[3], attack[4]
                dmg = projectile_damage(chosen[oid], index)
                if not dmg:
                    continue
                lo, hi = dmg[0] * count, dmg[1] * count
            parts.append("new PetStrike(%d, %d, %d, %g)" % (lo, hi, ms, rng))
        lines.append("            .Init(%s, new State(%s))" % (cs_string(oid), ", ".join(parts)))
    for oid in sorted(bs_pets):
        lines.append("            .Init(%s, new State(new PetFollow()))" % cs_string(oid))

    with open(PET_BEHAVIORS_CS, "w", encoding="utf-8", newline="\r\n") as f:
        f.write("// Generated by tools/port_doomed.py (port_doomed_pets.py). Do not edit.\n")
        f.write("// Permanent pets: the Doomed Realms pets with their healing and attacks (PetHeal, PetStrike),\n")
        f.write("// and betterSkillys' own permanent pets, which had no behaviour, with a follow.\n")
        f.write("using WorldServer.logic.behaviors;\nusing WorldServer.logic.behaviors.doomed;\n\n")
        f.write("namespace WorldServer.logic\n{\n    partial class BehaviorDb\n    {\n")
        f.write("        private _ DoomedPets = () => Behav()\n")
        f.write("\n".join(lines) + ";\n")
        f.write("    }\n}\n")
    return len(pets), len(bs_pets)
