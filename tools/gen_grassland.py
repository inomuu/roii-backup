#!/usr/bin/env python3
"""Procedural grassland ground for a whole BAND — one strip covering its 3 rooms.

    tools/gen_grassland.py --seed 7                    # 108x36 tiles -> art/bg_src/tiles_r2.png
    tools/gen_grassland.py --seed 7 --region 3         # write tiles_r3.png instead
    tools/gen_grassland.py --tmx ~/tilemap_r2_long.tmx # also write a Tiled map for hand-editing

Rebuilt (the first version lived in a session scratchpad and was lost with it — this one is
committed). Uses the ERW Grass Land tileset's own CORNER WANGSETS, the same auto-tiling Tiled
performs, so every feature edge gets the correct transition tile:

    base grass   = the 3x2 block 131..133 / 186..188 (+ sparse decor accents)
    dark grass   = wangset "grass2 - extra shade to main grass"
    dirt         = wangset "dirt"
    path         = wangset "stone floor", one winding road crossing all 3 rooms left-to-right
                   (the party's march line)

Different features keep a >= 2-tile gap (two transitions butted together render as a hard seam —
user: "transition จะเหลี่ยม ต้องเว้นหน่อย"). The strip is 108x36 tiles = 3456x1152 px: room
centres sit ~24 tiles apart (20 world units / 0.832), rooms at tiles ~30 / 54 / 78.

After this, bake the game background:  tools/build_bg.py 2 --raw
"""
import argparse
import os
import random
import sys
import xml.etree.ElementTree as ET

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TSX = os.path.expanduser(
    "~/Downloads/Tilesets/ERW - Grass Land 2.0 v2.1/TiledMap Editor/Tilesets/"
    "Tileset-Terrain-new grass.tsx")

BASE_BLOCK = [[131, 132, 133], [186, 187, 188]]   # gids (firstgid=1)
# (wangset name, wangcolor index) — a set holds SEVERAL terrain pairings as colours, and picking
# the wrong colour renders the wrong transition: dirt colour 1 is "dirt1 to EXTRA SHADE grass"
# (rings every patch with dark grass); colour 2 is the "to MAIN grass" pairing this map needs.
WS_DARK = ("grass2 - extra shade to main grass", 1)
WS_DIRT = ("dirt", 2)                    # dirt1 to main grass
WS_PATH = ("stone floor", 1)             # stone floor1 to main grass (style1)
# Flowers and tall grass are PROPS (billboards in the dressing scene), not baked ground — tried
# baked and cut: flat petals under the sprites' feet read worse than swaying billboards.


def _pillow():
    """Homebrew python's pyexpat is broken and lacks Pillow; the system python has both.
    Same re-exec build_bg.py does."""
    try:
        import PIL  # noqa: F401
        return
    except ImportError:
        pass
    sysp = "/usr/bin/python3"
    if sys.executable != sysp and os.path.exists(sysp):
        os.execv(sysp, [sysp] + sys.argv)
    raise SystemExit("Pillow not available")


def load_wangset(tsx_root, name, colour):
    """{(tr,br,bl,tl) 0/1 tuple: [tileid, ...]} for ONE wangcolour of a corner wangset —
    only tiles whose set corners are all that colour, so the transition pairs the right terrains."""
    for ws in tsx_root.find("wangsets"):
        if ws.get("name") != name:
            continue
        table = {}
        for wt in ws.findall("wangtile"):
            wid = [int(v) for v in wt.get("wangid").split(",")]
            corners = (wid[1], wid[3], wid[5], wid[7])  # TR BR BL TL
            if any(c not in (0, colour) for c in corners):
                continue
            key = tuple(1 if c == colour else 0 for c in corners)
            table.setdefault(key, []).append(int(wt.get("tileid")))
        return table
    raise SystemExit(f"wangset not found: {name}")


def blob(mask, cx, cy, r, rng, squish=0.6):
    """Paint a wobbly ellipse of corner-points into `mask` (a set of (x, y))."""
    for y in range(int(cy - r * squish) - 1, int(cy + r * squish) + 2):
        for x in range(int(cx - r) - 1, int(cx + r) + 2):
            dx, dy = x - cx, (y - cy) / squish
            if dx * dx + dy * dy <= r * r * (0.75 + rng.random() * 0.5):
                mask.add((x, y))


def near(mask, x, y, dist):
    for yy in range(y - dist, y + dist + 1):
        for xx in range(x - dist, x + dist + 1):
            if (xx, yy) in mask:
                return True
    return False


def mask_to_layer(mask, table, w, h, rng):
    """Corner mask -> tile layer via the wangset table (Tiled's corner auto-tiling)."""
    grid = [[0] * w for _ in range(h)]
    for y in range(h):
        for x in range(w):
            key = (int((x + 1, y) in mask), int((x + 1, y + 1) in mask),
                   int((x, y + 1) in mask), int((x, y) in mask))
            if key == (0, 0, 0, 0):
                continue
            tiles = table.get(key)
            if tiles:
                grid[y][x] = rng.choice(tiles) + 1  # tileid -> gid
    return grid


def gen(width, height, seed, back_pad=0):
    rng = random.Random(seed)
    tsx_root = ET.parse(TSX).getroot()
    t_dark = load_wangset(tsx_root, *WS_DARK)
    t_dirt = load_wangset(tsx_root, *WS_DIRT)
    t_path = load_wangset(tsx_root, *WS_PATH)

    height = height + back_pad   # pad rows are plain backdrop; features keep off them below
    # PATH: one road winding left->right across the whole strip. The drift is a slowly-steered
    # HEADING (not per-tile jitter, which staircased the edge every tile) and the width holds for
    # stretches, so the road reads as long soft curves.
    path = set()
    y = height * 0.55 + rng.uniform(-3, 3)
    heading = 0.0
    half = 1
    for x in range(-2, width + 3):
        heading = max(-0.35, min(0.35, heading + rng.uniform(-0.12, 0.12)))
        y = max(back_pad + 8.0, min(height - 8.0, y + heading))
        if rng.random() < 0.06:
            half = 1 + (rng.random() < 0.4)
        for dy in range(-half, half + 1):
            path.add((x, int(y) + dy))

    # DARK GRASS + DIRT: scattered blobs, >= 2-tile gap from the path and from each other.
    dark, dirt = set(), set()
    def scatter(mask, others, count, rmin, rmax):
        tries = 0
        while count > 0 and tries < 400:
            tries += 1
            cx, cy = rng.uniform(4, width - 4), rng.uniform(back_pad - 2, height - 4)
            r = rng.uniform(rmin, rmax)
            if any(near(o, int(cx), int(cy), int(r) + 2) for o in others):
                continue
            blob(mask, cx, cy, r, rng)
            count -= 1
    scatter(dark, [path, dirt], 11, 2.5, 5.0)
    scatter(dirt, [path, dark], 5, 2.0, 3.5)

    # BASE: the 3x2 grass block everywhere.
    base = [[BASE_BLOCK[y % 2][x % 3] for x in range(width)] for y in range(height)]

    return {
        "Grass": base,
        "DarkGrass": mask_to_layer(dark, t_dark, width, height, rng),
        "Dirt": mask_to_layer(dirt, t_dirt, width, height, rng),
        "Path": mask_to_layer(path, t_path, width, height, rng),
    }


def write_tmx(layers, width, height, out_path):
    rel = os.path.relpath(TSX, os.path.dirname(os.path.abspath(out_path)))
    parts = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<map version="1.10" orientation="orthogonal" renderorder="right-down" '
        f'width="{width}" height="{height}" tilewidth="32" tileheight="32" infinite="0" '
        f'nextlayerid="{len(layers) + 1}" nextobjectid="1">',
        f' <tileset firstgid="1" source="{rel}"/>',
    ]
    for i, (name, grid) in enumerate(layers.items(), 1):
        rows = ",\n".join(",".join(str(v) for v in row) for row in grid)
        parts += [f' <layer id="{i}" name="{name}" width="{width}" height="{height}">',
                  '  <data encoding="csv">', rows, '  </data>', ' </layer>']
    parts.append('</map>')
    with open(out_path, "w") as f:
        f.write("\n".join(parts))


def shade_pass(width, height, back_pad, seed):
    """Bake OCCLUSION into the ground — the half of the Octopath look a light rig cannot give a
    flat plane (key/ambient just scale a uniform surface and the tonemap eats it; measured).
    Returns a SHADE MAP (L image, value/1.2 encoded) that the ground shader multiplies in at
    runtime — a separate texture, not baked into the art, so it can be toggled/tuned live.
    Three layers, built at tile resolution and bilinear-upscaled so they stay soft:
      1. a shadow BAND under the cliff line (the ridge blocking the low sun),
      2. a progressive dim toward the far back rows (depth falloff),
      3. low-frequency dapple — broad light/shade pools so no stretch of grass is uniform."""
    from PIL import Image, ImageChops
    import random as _r
    rng = _r.Random(seed * 13 + 5)
    h = height
    shade = [[1.0] * width for _ in range(h)]
    # (2) back rows fall off toward the horizon
    for y in range(back_pad + 4):
        f = 0.72 + 0.28 * (y / float(back_pad + 4))
        for x in range(width):
            shade[y][x] *= f
    # (1) ridge shadow, CAST INTO THE FIELD: at the sun's -34 pitch a ~2.5-unit wall throws ~4-5
    # tiles of shade forward — so the band starts at the cliff base and falls off DOWNWARD (toward
    # the camera), across ground the player actually sees. The first version hid it behind the
    # cliffs and the change was invisible in game.
    base_row = back_pad + 13
    for dy in range(-1, 6):
        y = base_row + dy
        if 0 <= y < h:
            f = 0.72 + 0.05 * max(0, dy)         # darkest at the wall, fading out over 5 rows
            for x in range(width):
                shade[y][x] *= min(1.0, f + rng.uniform(-0.02, 0.02))
    # (1b) a dark FOREGROUND band along the bottom edge — the Octopath frame closes dark top AND
    # bottom, holding the eye in the lit middle.
    for i in range(6):
        y = h - 1 - i
        f = 0.80 + 0.033 * i
        for x in range(width):
            shade[y][x] *= f
    # (3) dapple: broad light/shade pools (one knot per ~8 tiles), and it may BRIGHTEN too — a
    # pool of sun next to a pocket of shade is the contrast, not uniform dimming.
    kw, kh = width // 8 + 2, h // 8 + 2
    knots = [[0.82 + rng.random() * 0.26 for _ in range(kw)] for _ in range(kh)]
    for y in range(h):
        for x in range(width):
            gx, gy = x / 8.0, y / 8.0
            x0, y0 = int(gx), int(gy)
            fx, fy = gx - x0, gy - y0
            v = (knots[y0][x0] * (1 - fx) * (1 - fy) + knots[y0][x0 + 1] * fx * (1 - fy)
                 + knots[y0 + 1][x0] * (1 - fx) * fy + knots[y0 + 1][x0 + 1] * fx * fy)
            shade[y][x] *= v
    # Values may exceed 1.0 (sun pools): encode f/1.2 into the L map, multiply, then restore the
    # 1.2 headroom — net effect img * f with true brightening.
    m = Image.new("L", (width, h))
    m.putdata([int(max(0.0, min(1.2, shade[y][x])) / 1.2 * 255)
               for y in range(h) for x in range(width)])
    return m.resize((width * 32, h * 32), Image.BILINEAR)


def match_tone(im, ref_path):
    """Shift the image's palette toward a reference bg, PER MATERIAL. The new-grass tileset's base
    green is teal next to the hand-approved bg_r1 warm green, but its stone and dirt already match
    — so a global transfer wrecked those (pink road) while a per-material mean shift only moves
    what differs. Classes are crisp on pixel art: grey = path, warm = dirt, rest = grass."""
    from PIL import Image
    def classify(c):
        mx, mn = max(c[:3]), min(c[:3])
        if mx - mn < 28:
            return "path"
        if c[0] > c[1]:
            return "dirt"
        return "grass"
    def stats(img):
        acc = {}
        for c in img.getdata():
            s = acc.setdefault(classify(c), [0.0, 0.0, 0.0, 0])
            for i in range(3):
                s[i] += c[i]
            s[3] += 1
        return {k: [s[i] / s[3] for i in range(3)] for k, s in acc.items()}
    ref = stats(Image.open(ref_path).convert("RGB").resize((480, 288)))
    src = stats(im.convert("RGB").resize((480, 288)))
    delta = {k: [ref[k][i] - src[k][i] for i in range(3)] for k in ref if k in src}
    px = im.load()
    for y in range(im.height):
        for x in range(im.width):
            c = px[x, y]
            d = delta.get(classify(c))
            if d:
                px[x, y] = tuple(
                    int(max(0, min(255, c[i] + d[i]))) for i in range(3)) + c[3:]
    return im


def render_png(layers, width, height, out_path):
    from PIL import Image
    root = ET.parse(TSX).getroot()
    img_el = root.find("image")
    sheet_path = os.path.normpath(os.path.join(os.path.dirname(TSX), img_el.get("source")))
    cols = int(root.get("columns"))
    sheet = Image.open(sheet_path).convert("RGBA")
    out = Image.new("RGBA", (width * 32, height * 32))
    cache = {}
    for grid in layers.values():
        for y in range(height):
            for x in range(width):
                gid = grid[y][x]
                if gid <= 0:
                    continue
                tid = gid - 1
                tile = cache.get(tid)
                if tile is None:
                    sx, sy = (tid % cols) * 32, (tid // cols) * 32
                    tile = sheet.crop((sx, sy, sx + 32, sy + 32))
                    cache[tid] = tile
                out.alpha_composite(tile, (x * 32, y * 32))
    return out


def save_png(out, width, height, out_path, tone_ref=""):
    if tone_ref:
        out = match_tone(out, tone_ref)
    out.save(out_path)
    print(f"wrote {out_path}  ({width}x{height} tiles, {width*32}x{height*32} px)")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--width", type=int, default=108)   # 3 rooms: centres ~ tiles 30/54/78
    ap.add_argument("--height", type=int, default=36)
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--region", type=int, default=2)
    ap.add_argument("--tmx", default="", help="also write a Tiled map here for hand-editing")
    ap.add_argument("--back-pad", type=int, default=10,
                    help="extra plain rows at the TOP (behind the cliffs) so the camera never sees past the art")
    ap.add_argument("--tone-ref", default="godot/assets/bg/bg_r1.png",
                    help="bg whose palette to match per-material ('' = keep the tileset's own)")
    args = ap.parse_args()
    _pillow()
    total_h = args.height + args.back_pad
    layers = gen(args.width, args.height, args.seed, args.back_pad)
    if args.tmx:
        write_tmx(layers, args.width, total_h, os.path.expanduser(args.tmx))
        print(f"wrote {args.tmx}")
    img = render_png(layers, args.width, total_h, None)
    ref = os.path.join(ROOT, args.tone_ref) if args.tone_ref else ""
    if ref:
        img = match_tone(img, ref)
    shade = shade_pass(args.width, total_h, args.back_pad, args.seed)
    sp = os.path.join(ROOT, "godot", "assets", "bg", f"bg_r{args.region}_s.png")
    shade.save(sp)
    print(f"wrote {sp} (runtime shade map)")
    save_png(img, args.width, total_h,
             os.path.join(ROOT, "art", "bg_src", f"tiles_r{args.region}.png"), "")


if __name__ == "__main__":
    main()
