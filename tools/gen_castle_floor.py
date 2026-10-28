#!/usr/bin/env python3
"""Bake the long castle FLOOR STRIP for the prologue zone -> godot/assets/bg/bg_r{REGION}.png.

The renderer maps a background ONCE over a plane sized from the image (no uv repeat), the way
`bg_r2` is one long baked grass strip rather than a tile (see docs/hd2d-look-pipeline.md §8). So
the castle floor is baked here too: the SAME aged-flagstone look the user picked in the proto
(tools/gen_castle_tex.py gen_floor — grout AO, broken tiles, hairline cracks, moss), extended over
a Band-2-sized canvas with per-tile variation + damage spread across the whole length so the tiling
never reads as an obvious repeat.

Deterministic (fixed seed). Outputs, matching bg_r2's 3456x1472 so the plane geometry lines up:
  bg_r{REGION}.png    albedo
  bg_r{REGION}_n.png  tangent-space normal (grout recesses catch the light)
  bg_r{REGION}_s.png  baked occlusion / shade (gentle: darker toward the back + a soft edge)

    python3 tools/gen_castle_floor.py            # region 90 (the reserved castle look), 3456x1472
    python3 tools/gen_castle_floor.py 90 3456 1472
"""
import math
import os
import random
import sys

from PIL import Image

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "godot", "assets", "bg")
T = 48            # flagstone size in px — dense enough that the grout grid reads crisp, not soft


def _vnoise(x, y, s):
    xi, yi = math.floor(x * s), math.floor(y * s)
    xf, yf = x * s - xi, y * s - yi

    def rnd(a, b):
        n = math.sin(a * 127.1 + b * 311.7) * 43758.5453
        return n - math.floor(n)
    a = rnd(xi, yi); b = rnd(xi + 1, yi); c = rnd(xi, yi + 1); d = rnd(xi + 1, yi + 1)
    u = xf * xf * (3 - 2 * xf); v = yf * yf * (3 - 2 * yf)
    return a * (1 - u) + b * u + (c - a) * v * (1 - u) + (d - b) * u * v


def gen(region, W, H):
    random.seed(23)
    BASE = [(140, 132, 116), (150, 142, 124), (128, 120, 106), (120, 114, 100)]
    GROUT = (70, 66, 58); RECESS = (52, 48, 42)
    MOSS = [(84, 104, 58), (98, 116, 68), (72, 92, 50)]
    cols = W // T + 1
    rows = H // T + 1

    def jit(c, a):
        return [max(0, min(255, ch + random.randint(-a, a))) for ch in c]

    def grout_at(x, y):
        return (x % T < 3) or (y % T < 3)

    # Damage spread over the whole strip, count scaled to tile area (~1242 tiles here).
    ntiles = cols * rows
    broken = set()
    for _ in range(ntiles // 30):
        broken.add((random.randint(0, cols), random.randint(0, rows)))
    crackpx = set()

    def gen_crack(x, y, n):
        for _ in range(n):
            crackpx.add((x, y))
            if random.random() < 0.35:
                crackpx.add((min(W - 1, x + 1), y))
            y += random.choice([1, 1, 1, 0]); x += random.choice([-1, -1, 0, 0, 0, 1, 1])
            x = max(0, min(W - 1, x)); y = max(0, min(H - 1, y))
            if random.random() < 0.06:
                gen_crack(x, y, random.randint(8, 22))
            if random.random() < 0.04:
                break
    for _ in range(ntiles // 40):
        gen_crack(random.randint(8, W - 8), random.randint(8, H - 8), random.randint(60, 140))
    blobs = [(random.randint(0, W), random.randint(0, H), random.uniform(30, 80))
             for _ in range(ntiles // 30)]

    alb = Image.new("RGBA", (W, H), (0, 0, 0, 255)); px = alb.load()
    hmap = [[1.0] * W for _ in range(H)]
    for y in range(H):
        for x in range(W):
            tx = x % T; ty = y % T
            tile = (x // T, y // T); grout = grout_at(x, y)
            base = jit(BASE[(tile[0] + tile[1] * 2) % 4], 8)
            if tile in broken:
                c = jit(RECESS, 12); hmap[y][x] = 0.15 + _vnoise(x, y, 0.2) * 0.2
                c = [max(0, min(255, ch + random.randint(-9, 9))) for ch in c]   # crisp grain
                px[x, y] = (c[0], c[1], c[2], 255)
                continue
            c = base
            dg = 99
            for r in range(1, 7):
                if grout_at(x + r, y) or grout_at(x - r, y) or grout_at(x, y + r) or grout_at(x, y - r):
                    dg = r; break
            if grout:
                c = jit(GROUT, 4); hmap[y][x] = 0.0
            else:
                # AO in the groove + a raised-stone BEVEL: lighter on the top/left inner edge,
                # darker on the bottom/right, so each flagstone reads as a defined block, not a
                # smooth patch. This is what stops the floor looking blurry.
                ao = 1.0 - max(0.0, (7 - dg)) / 7.0 * 0.40
                bev = 1.0
                if tx < 9: bev += (9 - tx) / 9.0 * 0.11
                if ty < 9: bev += (9 - ty) / 9.0 * 0.11
                if tx > T - 9: bev -= (tx - (T - 9)) / 9.0 * 0.15
                if ty > T - 9: bev -= (ty - (T - 9)) / 9.0 * 0.15
                c = [int(v * ao * bev) for v in c]; hmap[y][x] = min(1.0, dg / 4.0)
            # Milder large-scale grunge than before (0.85..1.05, not 0.66..1.16) so tiles stay
            # bright and defined instead of dissolving into big soft dark blooms.
            g = (_vnoise(x, y, 0.03) * 0.6 + _vnoise(x, y, 0.09) * 0.4); gf = 0.85 + g * 0.22
            for (bx, by, br) in blobs:
                d = math.hypot(x - bx, y - by)
                if d < br:
                    gf *= (0.86 + 0.14 * (d / br))
            c = [int(v * gf) for v in c]
            lum = int(0.3 * c[0] + 0.5 * c[1] + 0.2 * c[2]); c = [int(v * 0.82 + lum * 0.18) for v in c]
            # High-frequency per-pixel grain LAST, so nearest sampling shows crisp stone texel
            # variation up close rather than a smooth gradient.
            c = [max(0, min(255, ch + random.randint(-8, 8))) for ch in c]
            px[x, y] = (max(0, min(255, c[0])), max(0, min(255, c[1])), max(0, min(255, c[2])), 255)
    for (x, y) in crackpx:
        r, g, b, a = px[x, y]; px[x, y] = (int(r * 0.4), int(g * 0.4), int(b * 0.38), 255); hmap[y][x] = 0.0
    random.seed(2)
    for y in range(H):
        for x in range(W):
            near_damp = any(math.hypot(x - bx, y - by) < br * 0.7 for (bx, by, br) in blobs)
            if (grout_at(x, y) or (x, y) in crackpx or near_damp) and random.random() < 0.24:
                m = jit(random.choice(MOSS), 10); px[x, y] = (m[0], m[1], m[2], 255)
    alb.save(os.path.join(OUT, "bg_r%d.png" % region))
    print("wrote bg_r%d.png (%dx%d)" % (region, W, H))

    # normal (grout/cracks are recessed -> catch a rim of light)
    nm = Image.new("RGBA", (W, H), (128, 128, 255, 255)); nn = nm.load()

    def hh(x, y):
        return hmap[min(max(y, 0), H - 1)][min(max(x, 0), W - 1)]
    for y in range(H):
        for x in range(W):
            dx = (hh(x - 1, y) - hh(x + 1, y)) * 3.0; dy = (hh(x, y - 1) - hh(x, y + 1)) * 3.0
            l = math.sqrt(dx * dx + dy * dy + 1)
            nn[x, y] = (int((dx / l * .5 + .5) * 255), int((dy / l * .5 + .5) * 255),
                        int((1 / l * .5 + .5) * 255), 255)
    nm.save(os.path.join(OUT, "bg_r%d_n.png" % region))
    print("wrote bg_r%d_n.png" % region)

    # baked shade: gentle depth cue — darker toward the BACK (top rows, away from camera) and a
    # soft edge falloff; encoded L where the ground_recolour shader multiplies it in (values may
    # exceed the mid to brighten). Mostly ~mid so the floor keeps its own baked variation.
    sh = Image.new("RGBA", (W, H), (255, 255, 255, 255)); ss = sh.load()
    for y in range(H):
        back = 1.0 - 0.22 * (1.0 - y / H)          # top (back) rows dimmer
        for x in range(W):
            ex = 1.0 - 0.10 * (abs(x - W * 0.5) / (W * 0.5)) ** 2   # very soft side vignette
            f = max(0.0, min(1.0, back * ex))
            v = int(f * 255)
            ss[x, y] = (v, v, v, 255)
    sh.save(os.path.join(OUT, "bg_r%d_s.png" % region))
    print("wrote bg_r%d_s.png" % region)


if __name__ == "__main__":
    region = int(sys.argv[1]) if len(sys.argv) > 1 else 90
    W = int(sys.argv[2]) if len(sys.argv) > 2 else 3456
    H = int(sys.argv[3]) if len(sys.argv) > 3 else 1472
    os.makedirs(OUT, exist_ok=True)
    gen(region, W, H)
