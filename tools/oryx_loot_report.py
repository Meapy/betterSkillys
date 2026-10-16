"""What each stage of the Oryx chain drops, per player and kill, as the loot code rolls it.

    python tools/oryx_loot_report.py [--event 0.25] [--weekend 0.3]

Reads the loot tables of the chain's bosses from the behaviour files and works out the expected
drops of one player in a group of 1, 3 and 5 players who share the damage evenly. It follows
`Loot.Handle` (logic/loot/Loots.cs): each player rolls every entry whose threshold is below their
share of the damage, at the listed chance times 1 + their share + the loot boosts (the server-wide
event, weekends, a Loot Drop Potion), capped at certain. Spawned monsters drop nothing.
"""
import argparse
import glob
import os
import re
import xml.etree.ElementTree as ET

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DB = os.path.join(ROOT, "source", "WorldServer", "logic", "db")

STAGES = [
    ("1 Stone Guardians (both)", "BehaviorDb.OryxCastle.cs", ["Oryx Stone Guardian Right", "Oryx Stone Guardian Left"]),
    ("2 Janus", "BehaviorDb.OryxCastle.cs", ["md Janus the Doorwarden"]),
    ("3 Oryx 1", "BehaviorDb.Oryx.cs", ["Oryx the Mad God 1"]),
    ("4 Oryx 2", "BehaviorDb.Oryx.cs", ["Oryx the Mad God 2"]),
    ("5 Oryx 3", "BehaviorDb.OryxSanctuary.cs", ["Oryx the Mad God 3"]),
    ("6 Oryx 4", "BehaviorDb.OryxDoomed.cs", ["Oryx the Mad God 4"]),
    ("7 Oryx 5", "BehaviorDb.OryxDoomed.cs", ["Oryx the Mad God 5"]),
]

TEMPLATES = {
    "BasicDrop": [("T10 Weapon", .07), ("T11 Weapon", .07), ("T4 Ability", .07), ("T5 Ability", .07),
                  ("T11 Armor", .07), ("T12 Armor", .07), ("T4 Ring", .07), ("T5 Ring", .07)],
}
STATS = {"Potion of Attack", "Potion of Defense", "Potion of Speed", "Potion of Dexterity",
         "Potion of Vitality", "Potion of Wisdom"}
ORYX_SET = {"Ring of Oryx", "Oryx Hide Armor", "Oryx Hide Robe", "Oryx Scale Armor", "Oryx Tooth Sword",
            "Doom Dirk", "Flying Doom Daggers", "Tome of Noble Assault"}


def bag_types():
    """{item id: <BagType>} from the server's XML."""
    out = {}
    for f in glob.glob(os.path.join(ROOT, "source", "Shared", "resources", "xml", "**", "*.xml"), recursive=True):
        try:
            tree = ET.parse(f)
        except ET.ParseError:
            continue
        for o in tree.iter("Object"):
            if o.findtext("BagType"):
                out[o.get("id")] = int(o.findtext("BagType"))
    return out


BAGS = None


def loot_of(path, name):
    """[(item, chance, threshold)] of one Init."""
    s = open(path, encoding="utf-8-sig").read()
    a = s.index('.Init("%s"' % name)
    b = s.index("\n            )", s.index("new Threshold(", a))
    out, thr = [], 0.0
    for line in s[a:b].splitlines():
        m = re.search(r"new Threshold\(([\d.]+)", line)
        if m:
            thr = float(m.group(1))
        m = re.search(r'new ItemLoot\("([^"]+)", ([\d.]+)', line)
        if m:
            out.append((m.group(1), float(m.group(2)), thr))
        m = re.search(r"new TierLoot\((\d+), ItemType\.(\w+), ([\d.]+)", line)
        if m:
            out.append(("T%s %s" % (m.group(1), m.group(2)), float(m.group(3)), thr))
        m = re.search(r"LootTemplates\.(\w+)\(\)", line)
        if m:
            out += [(i, p, thr) for i, p in TEMPLATES[m.group(1)]]
    return out


def tier(item):
    m = re.match(r"T(\d+) (\w+)", item)
    return (int(m.group(1)), m.group(2)) if m else None


def report(group, boost):
    share = 1.0 / group
    mult = 1 + share + boost
    print("\n%d player%s (each %.0f%% of the damage): chances x%.2f" % (group, "" if group == 1 else "s", share * 100, mult))
    print("  %-26s %6s %5s %5s %6s %6s %7s %8s" % ("stage", "stats", "life", "mana", "oryx", "gear", "topgear", "white"))
    for label, f, names in STAGES:
        totals = dict(stats=0.0, life=0.0, mana=0.0, oryx=0.0, gear=0.0, top=0.0, special=0.0)
        for n in names:
            for item, p, thr in loot_of(os.path.join(DB, f), n):
                if thr >= share:     # Loots.cs: Threshold < share of the damage
                    continue
                e = min(1.0, p * mult)
                t = tier(item)
                if item in STATS:
                    totals["stats"] += e
                elif item in ("Potion of Life", "Greater Potion of Life"):
                    totals["life"] += e
                elif item in ("Potion of Mana", "Greater Potion of Mana"):
                    totals["mana"] += e
                elif item == "Potion of Oryx":
                    totals["oryx"] += e
                elif t:
                    top = (t[1] in ("Weapon", "Armor") and t[0] >= 13) or (t[1] in ("Ability", "Ring") and t[0] >= 6)
                    totals["top" if top else "gear"] += e
                elif BAGS.get(item) == 6:
                    # chance of at least one white-bag item
                    totals["special"] = 1 - (1 - totals["special"]) * (1 - e)
        print("  %-26s %6.2f %5.2f %5.2f %5.0f%% %6.2f %7.2f %7.1f%%" % (
            label, totals["stats"], totals["life"], totals["mana"], totals["oryx"] * 100,
            totals["gear"], totals["top"], totals["special"] * 100))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--event", type=float, default=0.25, help="serverSettings.lootEvent (live: 0.25)")
    ap.add_argument("--weekend", type=float, default=0.0, help="serverSettings.wkndBoost on a weekend (live: 0.3)")
    args = ap.parse_args()
    global BAGS
    BAGS = bag_types()
    print("Expected drops per player and kill. stats: the six stat potions; life/mana include the Greater ones;")
    print("oryx: Potion of Oryx; gear / topgear: tiered items (top: T13+ weapons and armour, T6 abilities and")
    print("rings); white: chance of at least one white-bag item (the Oryx set, Oryx's Escutcheon, seals...).")
    for group in (1, 3, 5):
        report(group, args.event + args.weekend)


if __name__ == "__main__":
    main()
