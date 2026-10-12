"""Builds the client's Elder dungeon sprite sheet (custom/images/ElderAssets8x8.png).

The Elder dungeons reuse the look of their regular dungeon in a darker shade: each sprite here
is a copy of a betterSkillys sprite with a shade applied (the floor dark stone, the walls aged
gold, so the walls still stand out against the floor). The ground and object
definitions that use the sheet ("elderAssets8x8") are in source/Shared/resources/xml/custom/
CustomGrounds.xml and CustomObjects.xml, and their client copies.

    python tools/make_elder_assets.py
"""

import os
import struct
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
IMAGES = os.path.join(ROOT, "client", "src", "kabam", "rotmg", "assets", "prod", "images")
OUT = os.path.join(ROOT, "client", "src", "kabam", "rotmg", "assets", "custom", "images", "ElderAssets8x8.png")

FLOOR = (0.4, 0.36, 0.42)   # red, green and blue multipliers
WALL = (0.75, 0.6, 0.42)
SIZE = 8
PER_ROW = 16

# sheet index -> (source sheet, source index, shade)
SPRITES = {}
# Elder Tomb grounds: Sandstone Tile, Cracked Sandstone Tile (also Sand Covered Tile), Missing Tile, Gold Tile
SPRITES.update({0x00: ("lofiEnvironment2", 0x03, FLOOR), 0x01: ("lofiEnvironment2", 0x06, FLOOR),
                0x02: ("lofiEnvironment2", 0x04, FLOOR), 0x03: ("lofiEnvironment2", 0x05, FLOOR)})
# Elder Tomb Wall: the Tomb Wall's 29 random faces, then its dot, short line, L, line, T and cross tops
SPRITES.update({0x20 + i: ("lofiEnvironment3", i, WALL) for i in range(0x1d)})
SPRITES.update({0x40 + i: ("lofiEnvironment3", 0x30 + i, WALL) for i in range(6)})


def read_png(path):
    data = open(path, "rb").read()
    pos, chunks = 8, []
    while pos < len(data):
        n, t = struct.unpack(">I4s", data[pos:pos + 8])
        chunks.append((t, data[pos + 8:pos + 8 + n]))
        pos += 12 + n
    ihdr = next(d for t, d in chunks if t == b"IHDR")
    w, h, depth, ctype = struct.unpack(">IIBB", ihdr[:10])
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


def main():
    sheets = {}
    rows = (max(SPRITES) // PER_ROW + 1)
    w, h = PER_ROW * SIZE, max(rows * SIZE, 128)
    pix = [[(0, 0, 0, 0)] * w for _ in range(h)]
    for idx, (sheet, src, shade) in SPRITES.items():
        if sheet not in sheets:
            sheets[sheet] = read_png(os.path.join(IMAGES, sheet + ".png"))
        sw, _, px = sheets[sheet]
        sx, sy = (src % (sw // SIZE)) * SIZE, (src // (sw // SIZE)) * SIZE
        dx, dy = (idx % PER_ROW) * SIZE, (idx // PER_ROW) * SIZE
        for y in range(SIZE):
            for x in range(SIZE):
                r, g, b, a = px(sx + x, sy + y)
                pix[dy + y][dx + x] = (int(r * shade[0]), int(g * shade[1]), int(b * shade[2]), a)
    write_png(OUT, w, h, pix)
    print("Wrote %s: %d sprites." % (OUT, len(SPRITES)))


if __name__ == "__main__":
    main()
