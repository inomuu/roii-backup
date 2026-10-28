#!/usr/bin/env python3
"""Bake a 1px black outline onto the Comistion humanoid monster sheets so they
match the Tiny RPG player sprites (which carry a black pixel-art outline). The
Comistion art has none, which read as "from a different game".

Source = the untouched Comistion K strips in Downloads (so re-running is clean);
output = godot/assets/monsters/<name>.png. Outline is computed PER FRAME (10x4
grid of 128x96 cells) so it can never bleed across frame boundaries.
"""
from PIL import Image

SRC_DIR = "/Users/lunalieneray/Downloads/Comistion K"
OUT_DIR = "godot/assets/monsters"
# Comistion strip number -> in-game sheet name.
NAMES = {1: "squire", 2: "archer", 3: "shielder", 4: "dummy", 5: "cleric",
         6: "wizard", 7: "berserker", 8: "knight", 9: "crossbowman",
         10: "leader", 11: "darknight"}
COLS, ROWS = 10, 4
NBRS = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)]


def outline_cell(im):
    """Return a copy of `im` with black on every transparent pixel that touches
    an opaque one (1px, 8-connected)."""
    im = im.convert("RGBA")
    w, h = im.size
    px = im.load()
    op = [[px[x, y][3] > 40 for x in range(w)] for y in range(h)]
    out = im.copy()
    o = out.load()
    for y in range(h):
        for x in range(w):
            if op[y][x]:
                continue
            for dx, dy in NBRS:
                nx, ny = x + dx, y + dy
                if 0 <= nx < w and 0 <= ny < h and op[ny][nx]:
                    o[x, y] = (0, 0, 0, 255)
                    break
    return out


def process(num, name):
    sheet = Image.open(f"{SRC_DIR}/{num}.png").convert("RGBA")
    W, H = sheet.size
    fw, fh = W // COLS, H // ROWS
    res = Image.new("RGBA", (W, H))
    for ry in range(ROWS):
        for cx in range(COLS):
            box = (cx * fw, ry * fh, cx * fw + fw, ry * fh + fh)
            res.paste(outline_cell(sheet.crop(box)), box)
    res.save(f"{OUT_DIR}/{name}.png")


if __name__ == "__main__":
    for num, name in NAMES.items():
        process(num, name)
        print(f"outlined {name}.png")
