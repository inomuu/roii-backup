#!/usr/bin/env python3
"""Generate the ruined-castle texture set (godot/assets/castle/).

Deterministic (fixed seeds): re-running reproduces the checked-in PNGs byte-for-byte, so the
textures are edited HERE, never in an image editor. Recovered from the 2026-09-03 proto session.

    python3 tools/gen_castle_tex.py            # all
    python3 tools/gen_castle_tex.py floor wall # a subset

Outputs (all pixel-art, NEAREST-filtered in Godot):
  stone_floor.png / _n.png   128x128, 4x4 tiles of 32px — aged, cracked, mossy; tiles (~2.6 u/rep)
  stone_block.png / _n.png    96x128, running-bond ashlar; periodic grunge so it tiles both axes
  castle_wall.png            120x152, one upright ruined wall slab with battlements (alpha silhouette)
  carpet.png / _n.png         64x48, red runner with gold trim; tile along v
  banner.png                  40x96, hanging crimson banner, tattered V bottom (alpha)
  flame.png                   32x48, soft additive torch flame
"""
import math
import os
import random
import sys

from PIL import Image

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "godot", "assets", "castle")


def _save(im, name):
    p = os.path.join(OUT, name)
    im.save(p)
    print("wrote", os.path.relpath(p))


def _normal_from_height(hmap, W, H, strength, wrap):
    """Tangent-space normal map from a 2-D height list. wrap=True samples toroidally (tileable)."""
    def hh(x, y):
        if wrap:
            return hmap[y % H][x % W]
        return hmap[min(max(y, 0), H - 1)][min(max(x, 0), W - 1)]
    nm = Image.new("RGBA", (W, H), (128, 128, 255, 255))
    nn = nm.load()
    for y in range(H):
        for x in range(W):
            dx = (hh(x - 1, y) - hh(x + 1, y)) * strength
            dy = (hh(x, y - 1) - hh(x, y + 1)) * strength
            l = math.sqrt(dx * dx + dy * dy + 1)
            nn[x, y] = (int((dx / l * .5 + .5) * 255), int((dy / l * .5 + .5) * 255),
                        int((1 / l * .5 + .5) * 255), 255)
    return nm


def _vnoise(x, y, s):
    xi, yi = math.floor(x * s), math.floor(y * s)
    xf, yf = x * s - xi, y * s - yi

    def rnd(a, b):
        n = math.sin(a * 127.1 + b * 311.7) * 43758.5453
        return n - math.floor(n)
    a = rnd(xi, yi); b = rnd(xi + 1, yi); c = rnd(xi, yi + 1); d = rnd(xi + 1, yi + 1)
    u = xf * xf * (3 - 2 * xf); v = yf * yf * (3 - 2 * yf)
    return a * (1 - u) + b * u + (c - a) * v * (1 - u) + (d - b) * u * v


# --- floor: the "first aged 128" look the user picked (grout AO, broken tiles, cracks, moss) ---
def gen_floor():
    random.seed(23)
    W, H, T = 128, 128, 32
    BASE = [(140, 132, 116), (150, 142, 124), (128, 120, 106), (120, 114, 100)]
    GROUT = (70, 66, 58); RECESS = (52, 48, 42)
    MOSS = [(84, 104, 58), (98, 116, 68), (72, 92, 50)]

    def jit(c, a):
        return [max(0, min(255, ch + random.randint(-a, a))) for ch in c]

    def grout_at(x, y):
        return (x % T < 2) or (y % T < 2)
    broken = set()
    for _ in range(6):
        broken.add((random.randint(0, W // T), random.randint(0, H // T)))
    crackpx = set()

    def gen_crack(x, y, n):
        for _ in range(n):
            crackpx.add((x, y))
            if random.random() < 0.35:
                crackpx.add((min(W - 1, x + 1), y))
            y += random.choice([1, 1, 1, 0]); x += random.choice([-1, -1, 0, 0, 0, 1, 1])
            x = max(0, min(W - 1, x)); y = max(0, min(H - 1, y))
            if random.random() < 0.07:
                gen_crack(x, y, random.randint(6, 16))
            if random.random() < 0.05:
                break
    for _ in range(5):
        gen_crack(random.randint(8, W - 8), random.randint(0, 20), random.randint(40, 90))
    blobs = [(random.randint(0, W), random.randint(0, H), random.uniform(18, 40)) for _ in range(7)]
    alb = Image.new("RGBA", (W, H), (0, 0, 0, 255)); px = alb.load()
    hmap = [[1.0] * W for _ in range(H)]
    for y in range(H):
        for x in range(W):
            tile = (x // T, y // T); grout = grout_at(x, y)
            base = jit(BASE[(tile[0] + tile[1] * 2) % 4], 10)
            if tile in broken:
                c = jit(RECESS, 12); hmap[y][x] = 0.15 + _vnoise(x, y, 0.4) * 0.2
                px[x, y] = (c[0], c[1], c[2], 255)
                continue
            c = base
            dg = 99
            for r in range(1, 5):
                if grout_at(x + r, y) or grout_at(x - r, y) or grout_at(x, y + r) or grout_at(x, y - r):
                    dg = r; break
            if grout:
                c = jit(GROUT, 5); hmap[y][x] = 0.0
            else:
                ao = 1.0 - max(0.0, (5 - dg)) / 5.0 * 0.42
                c = [int(v * ao) for v in c]; hmap[y][x] = min(1.0, dg / 3.0)
            g = (_vnoise(x, y, 0.06) * 0.6 + _vnoise(x, y, 0.18) * 0.4); gf = 0.66 + g * 0.5
            for (bx, by, br) in blobs:
                d = math.hypot(x - bx, y - by)
                if d < br:
                    gf *= (0.72 + 0.28 * (d / br))
            c = [max(0, min(255, int(v * gf))) for v in c]
            lum = int(0.3 * c[0] + 0.5 * c[1] + 0.2 * c[2]); c = [int(v * 0.7 + lum * 0.3) for v in c]
            c = [int(c[0] * 1.02), int(c[1] * 0.99), int(c[2] * 0.93)]
            px[x, y] = (max(0, min(255, c[0])), max(0, min(255, c[1])), max(0, min(255, c[2])), 255)
    for (x, y) in crackpx:
        r, g, b, a = px[x, y]; px[x, y] = (int(r * 0.4), int(g * 0.4), int(b * 0.38), 255); hmap[y][x] = 0.0
    random.seed(2)
    for y in range(H):
        for x in range(W):
            near_damp = any(math.hypot(x - bx, y - by) < br * 0.7 for (bx, by, br) in blobs)
            if (grout_at(x, y) or (x, y) in crackpx or near_damp) and random.random() < 0.28:
                m = jit(random.choice(MOSS), 10); px[x, y] = (m[0], m[1], m[2], 255)
    _save(alb, "stone_floor.png")
    _save(_normal_from_height(hmap, W, H, 3.0, wrap=False), "stone_floor_n.png")


# --- stone block (walls, columns, rubble): tileable both axes, per-brick deterministic jitter ---
def gen_block():
    random.seed(31)
    W, H, BW, BH = 96, 128, 32, 16   # 3 cols x 8 rows; 8 rows even -> running bond wraps
    SB = [(138, 130, 114), (128, 120, 104), (148, 140, 122), (120, 112, 98)]
    MJ = (74, 70, 62)

    def jit(c, a, rng):
        return [max(0, min(255, ch + rng.randint(-a, a))) for ch in c]

    def brick_col(col, row):
        r = random.Random(col * 131 + row * 977)
        return jit(SB[(col + row) % 4], 11, r)

    def is_grout(x, y):
        row = y // BH; off = (BW // 2) if row % 2 else 0
        return ((x + off) % BW < 2) or (y % BH < 2)

    def periodic_grunge(x, y):
        fx = x / W * 2 * math.pi; fy = y / H * 2 * math.pi
        g = 0.5 + 0.22 * math.sin(2 * fx + 1.3) * math.sin(2 * fy + 0.7) + 0.14 * math.sin(3 * fx + 2.1) * math.sin(fy + 4.0)
        return 0.70 + g * 0.42
    alb = Image.new("RGBA", (W, H), (0, 0, 0, 255)); px = alb.load()
    hmap = [[1.0] * W for _ in range(H)]
    for y in range(H):
        for x in range(W):
            row = y // BH; off = (BW // 2) if row % 2 else 0; col = (x + off) // BW
            if is_grout(x, y):
                r = random.Random(x * 7 + y * 3); c = jit(MJ, 5, r); hmap[y][x] = 0.0
            else:
                c = brick_col(col, row)
                dg = 99
                for rr in range(1, 5):
                    if is_grout(x + rr, y) or is_grout(x - rr, y) or is_grout(x, y + rr) or is_grout(x, y - rr):
                        dg = rr; break
                ao = 1.0 - max(0.0, (5 - dg)) / 5.0 * 0.4
                c = [int(v * ao) for v in c]; hmap[y][x] = min(1.0, dg / 3.0)
            gf = periodic_grunge(x, y); c = [int(v * gf) for v in c]
            lum = int(0.3 * c[0] + 0.5 * c[1] + 0.2 * c[2]); c = [int(v * 0.78 + lum * 0.22) for v in c]
            px[x, y] = (max(0, min(255, c[0])), max(0, min(255, c[1])), max(0, min(255, c[2])), 255)
    _save(alb, "stone_block.png")
    _save(_normal_from_height(hmap, W, H, 3.0, wrap=True), "stone_block_n.png")


# --- upright ruined wall slab: battlements, collapsed right end, hairline cracks, moss base ---
def gen_wall():
    random.seed(11)
    W, H = 120, 152
    im = Image.new("RGBA", (W, H), (0, 0, 0, 0)); px = im.load()
    STONE = [(126, 118, 102), (138, 130, 112), (112, 104, 90), (148, 140, 120), (120, 110, 96)]
    MORTAR = (74, 70, 62); RECESS = (52, 48, 42); MOSS = [(84, 104, 58), (98, 116, 68), (72, 92, 50)]

    def jit(c, a=12):
        return tuple(max(0, min(255, ch + random.randint(-a, a))) for ch in c)
    BH = 11; BW = 17; top_wall = 26
    missing = set()
    for _ in range(5):
        missing.add((random.randint(2, H // BH - 1), random.randint(0, W // BW)))
    for y in range(top_wall, H):
        for x in range(W):
            row = y // BH; off = (BW // 2) if (row % 2) else 0; bx = (x + off) // BW
            on_v = ((x + off) % BW == 0); on_h = (y % BH == 0); base = STONE[(row * 3 + bx) % len(STONE)]
            c = jit(RECESS, 6) if (row, bx) in missing else (jit(MORTAR, 6) if (on_v or on_h) else jit(base, 12))
            px[x, y] = (c[0], c[1], c[2], 255)
    merlon_w = 20; gap_w = 12; period = merlon_w + gap_w
    for x in range(W):
        if (x % period) < merlon_w:
            for y in range(0, top_wall):
                row = y // BH; base = STONE[(x // BW + row) % len(STONE)]
                on_v = (x % BW == 0); on_h = (y % BH == 0)
                c = jit(MORTAR, 6) if (on_v or on_h) else jit(base, 12)
                px[x, y] = (c[0], c[1], c[2], 255)
    random.seed(3); cy = top_wall + 4
    for x in range(W - 1, W - 46, -1):
        cy = max(top_wall - 6, min(H - 40, cy + random.choice([-3, -2, -1, 0, 1, 2, 3, 4])))
        for y in range(0, cy):
            px[x, y] = (0, 0, 0, 0)

    def darken(x, y, f):
        if 0 <= x < W and 0 <= y < H and px[x, y][3] > 0:
            r, g, b, a = px[x, y]; px[x, y] = (int(r * f), int(g * f), int(b * f), 255)
    random.seed(21)
    for _ in range(3):
        x = random.randint(14, W - 24); y = random.randint(top_wall + 2, top_wall + 16)
        steps = random.randint(28, 52)
        for _ in range(steps):
            darken(x, y, 0.62)
            if random.random() < 0.25:
                darken(x + 1, y, 0.78)
            y += random.choice([1, 1, 1, 0]); x += random.choice([-1, 0, 0, 1])
            if random.random() < 0.06:
                bx, by = x, y
                for _ in range(random.randint(4, 9)):
                    darken(bx, by, 0.66); bx += random.choice([-1, 0, 1]); by += random.choice([0, 1])
            if random.random() < 0.08:
                break
    for y in range(H - 46, H):
        for x in range(W):
            if px[x, y][3] == 0:
                continue
            d = (y - (H - 46)) / 46.0
            if random.random() < 0.16 * d:
                m = jit(random.choice(MOSS), 10); px[x, y] = (m[0], m[1], m[2], 255)
    for x in range(W):
        if (x % period) >= merlon_w:
            for y in range(top_wall, min(top_wall + 34, H)):
                if px[x, y][3] > 0 and random.random() < 0.4:
                    r, g, b, a = px[x, y]; px[x, y] = (int(r * 0.86), int(g * 0.86), int(b * 0.83), 255)
    _save(im, "castle_wall.png")


# --- red carpet runner: u = across, v = along (tile v) ---
def gen_carpet():
    random.seed(4)
    W, Hh = 64, 48
    im = Image.new("RGBA", (W, Hh), (0, 0, 0, 255)); px = im.load()
    RED = [(146, 32, 36), (128, 26, 30), (158, 40, 42)]
    GOLD = (184, 156, 92); GOLD_D = (120, 96, 44); EDGE = (52, 20, 20)

    def jit(c, a=8):
        return tuple(max(0, min(255, ch + random.randint(-a, a))) for ch in c)
    for y in range(Hh):
        for x in range(W):
            if x < 2 or x >= W - 2:
                c = EDGE
            elif x < 8 or x >= W - 8:
                c = jit(GOLD, 10) if (x == 5 or x == W - 6 or (x % 2 == 0)) else jit(GOLD_D, 10)
            elif x == 8 or x == W - 9:
                c = GOLD_D
            else:
                c = jit(RED[(x + y) % 3], 6)
            px[x, y] = (c[0], c[1], c[2], 255)
    cx = W // 2; cy = Hh // 2; s = 9
    for y in range(Hh):
        for x in range(10, W - 10):
            d = abs(x - cx) + abs(y - cy)
            if s - 1 <= d <= s:
                g = jit(GOLD, 8); px[x, y] = (g[0], g[1], g[2], 255)
            elif d < s - 1 and (d == s - 4):
                g = jit(GOLD_D, 8); px[x, y] = (g[0], g[1], g[2], 255)
    _save(im, "carpet.png")

    def isgold(x, y):
        r, g, b, a = px[x, y]; return g > 90 and r > 140 and b < 120
    hmap = [[1.0 if isgold(x, y) else 0.4 for x in range(W)] for y in range(Hh)]
    _save(_normal_from_height(hmap, W, Hh, 2.0, wrap=False), "carpet_n.png")


# --- hanging banner: crimson, gold trim, diamond emblem, tattered V bottom + holes ---
def gen_banner():
    # Hanging tattered banner: crimson + gold trim + diamond emblem, an ORGANIC frayed bottom hem
    # (per-column torn length + noise + deep rips, no blocky V), a few ragged holes, dangling threads.
    # Higher res (132x300) so the alpha-scissor torn edge reads fine, not pixel-blocky.
    random.seed(77)
    W, H = 132, 300
    im = Image.new("RGBA", (W, H), (0, 0, 0, 0)); px = im.load()
    FIELD = [(150, 34, 40), (128, 26, 32), (166, 44, 50)]
    GOLD = (194, 162, 92); GOLD_D = (128, 100, 46)

    def jit(c, a=8):
        return tuple(max(0, min(255, ch + random.randint(-a, a))) for ch in c)

    lowf = [random.uniform(0, 1) for _ in range(8)]
    col_base = []
    for x in range(W):
        t = x / W * (len(lowf) - 1)
        i = int(t); f = t - i
        smooth = lowf[i] * (1 - f) + lowf[min(i + 1, len(lowf) - 1)] * f
        col_base.append(H - 40 - smooth * 70 - random.random() * 14)
    for _ in range(5):
        rx = random.randint(6, W - 6); depth = random.randint(30, 80); wdt = random.randint(1, 4)
        for x in range(max(0, rx - wdt), min(W, rx + wdt)):
            col_base[x] = min(col_base[x], H - 30 - depth + random.random() * 10)
    holes = [(random.randint(20, W - 20), random.randint(H // 2, H - 60), random.uniform(6, 13)) for _ in range(3)]

    for y in range(H):
        for x in range(W):
            if y > col_base[x]:
                continue
            skip = False
            for (hx, hy, hr) in holes:
                if math.hypot(x - hx, (y - hy) * 1.3) < hr * (0.75 + 0.35 * math.sin(x * 0.7 + y * 0.5)):
                    skip = True; break
            if skip:
                continue
            if x < 4 or x >= W - 4:
                c = jit(GOLD_D)
            elif x < 9 or x >= W - 9:
                c = jit(GOLD, 10)
            else:
                c = jit(FIELD[(x + y) % 3], 7)
            near = col_base[x] - y
            if near < 26:
                fade = 0.6 + 0.4 * (near / 26.0)
                c = tuple(int(v * fade) for v in c)
            px[x, y] = (c[0], c[1], c[2], 255)

    cx, cy = W // 2, 60
    for y in range(H):
        for x in range(W):
            if px[x, y][3] == 0:
                continue
            d = abs(x - cx) + abs(y - cy) * 1.1
            if 16 <= d <= 20:
                g = jit(GOLD); px[x, y] = (g[0], g[1], g[2], 255)
            elif d <= 8:
                g = jit(GOLD_D); px[x, y] = (g[0], g[1], g[2], 255)

    random.seed(9)
    for x in range(6, W - 6):
        if random.random() < 0.10:
            by = int(col_base[x]); length = random.randint(4, 16)
            for k in range(length):
                yy = by + k
                if 0 <= yy < H:
                    c = jit(FIELD[x % 3], 5)
                    px[max(0, min(W - 1, x + int(math.sin(k * 0.6) * 1.5))), yy] = (c[0], c[1], c[2], 255)
    _save(im, "banner.png")


# --- torch flame: soft radial gradient, alpha^2 falloff, meant for BLEND_MODE_ADD ---
def gen_flame():
    W, H = 32, 48
    im = Image.new("RGBA", (W, H), (0, 0, 0, 0)); p = im.load()
    for y in range(H):
        for x in range(W):
            dx = (x - W / 2) / (W / 2); dy = (y - H * 0.62) / (H * 0.5)
            d = (dx * dx + dy * dy * 0.6)
            a = max(0.0, 1.0 - d)
            a *= (1.0 - abs(dx) * 0.3)
            col = (255, int(180 + 70 * a), int(60 + 40 * a))
            p[x, y] = (col[0], min(255, col[1]), min(255, col[2]), int(255 * a * a))
    _save(im, "flame.png")


GENS = {"floor": gen_floor, "block": gen_block, "wall": gen_wall, "carpet": gen_carpet,
        "banner": gen_banner, "flame": gen_flame}

if __name__ == "__main__":
    want = sys.argv[1:] or list(GENS)
    bad = [w for w in want if w not in GENS]
    if bad:
        sys.exit("unknown: %s (choose from %s)" % (bad, ", ".join(GENS)))
    os.makedirs(OUT, exist_ok=True)
    for w in want:
        GENS[w]()
