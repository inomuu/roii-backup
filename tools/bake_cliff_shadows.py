#!/usr/bin/env python3
"""Bake the CLIFF ground shadows into godot/assets/bg/bg_r2_s.png, aligned to the actual cliffs.

The back-tier cliffs (CliffC*) were hand-placed into zone_dressing_r2.tscn AFTER the grass shade
map was generated, so the baked shade had no shadow for them and the cliffs read as floating
(user 2026-09-04). This reads each Cliff* node's world footprint + height straight from the .tscn
and casts a soft shadow onto the ground in the sun direction, compositing it over the grass-only
base shade (art/bg_src/bg_r2_s_base.png) — so re-running is idempotent and edits to cliff placement
just re-bake correctly.

    python3 tools/bake_cliff_shadows.py

Tunables (UV orientation of the shade sampler + sun projection) are constants at the top; verify by
re-rendering proto/arena3d/dressing_preview.tscn and checking the shadow sits at each cliff base.
"""
import math
import os
import re

from PIL import Image, ImageDraw, ImageFilter

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
TSCN = os.path.join(ROOT, "godot", "proto", "arena3d", "zone_dressing_r2.tscn")
BASE = os.path.join(ROOT, "art", "bg_src", "bg_r2_s_base.png")   # grass-only shade
OUT = os.path.join(ROOT, "godot", "assets", "bg", "bg_r2_s.png")

TILE = 0.832
DRESS_Z = -2.0            # dressing root z (FIELD_Z_SHIFT), added to every node's local z
# The ground plane, exactly as the arena/dressing_preview place it.
PLANE_CX, PLANE_CZ = 20.0, -6.16          # centre world x,z  (FIELD_Z_SHIFT + BG_Z_SHIFT[2] = -2 + -4.16)
PLANE_W, PLANE_H = 89.856, 38.272         # world size (= 3456*0.026, 1472*0.026)

# --- UV orientation of the shade sampler (verified by re-rendering) ---
U_FLIP = False           # flip world-x -> pixel-x
V_FLIP = True            # image row 0 = the BACK (min z, the cliff line); grass shade is authored that way

# --- sun projection: a cliff of height h throws its shadow this far, per world unit of height ---
# Sun pitch -34 -> length = 1/tan(34) ≈ 1.48; it falls toward the FIELD (+z) with an x lean from the
# -38 yaw. Tuned against the render.
SHADOW_DZ_PER_H = 1.15   # forward, into the field (+z)
SHADOW_DX_PER_H = 0.60   # lateral lean from the yaw
DARK = 0.55              # shadowed ground multiplier (lower = darker)
BLUR_PX = 22             # soft edge
FOOT_PAD = 0.4           # widen the footprint a touch so the base of the wall is covered too


def parse_cliffs():
    cliffs = []
    cur = None
    for line in open(TSCN, encoding="utf-8"):
        m = re.match(r'\[node name="(Cliff[^"]+)"', line)
        if m:
            if cur:
                cliffs.append(cur)
            cur = {"name": m.group(1), "w": 4.0, "d": 3.0, "h": 3.0}
            continue
        if cur is None:
            continue
        mt = re.match(r"transform = Transform3D\(([^)]+)\)", line)
        if mt:
            v = [float(x) for x in mt.group(1).split(",")]
            cur["x"], cur["z"] = v[9], v[11]     # translation x, z
        for key, tag in (("w", "width_tiles"), ("d", "depth_tiles"), ("h", "height_tiles")):
            mk = re.match(r"%s = ([\d.]+)" % tag, line)
            if mk:
                cur[key] = float(mk.group(1))
        if line.strip() == "" and cur:
            cliffs.append(cur)
            cur = None
    if cur:
        cliffs.append(cur)
    return [c for c in cliffs if "x" in c]


def world_to_px(x, z, W, H):
    u = (x - (PLANE_CX - PLANE_W / 2.0)) / PLANE_W
    v = (z - (PLANE_CZ - PLANE_H / 2.0)) / PLANE_H
    if U_FLIP:
        u = 1.0 - u
    if V_FLIP:
        v = 1.0 - v
    return u * W, v * H


def main():
    base = Image.open(BASE).convert("L")
    W, H = base.size
    cliffs = parse_cliffs()
    print("baking %d cliff shadows into %dx%d shade" % (len(cliffs), W, H))

    # Paint all shadows onto a white mask (0 = full shadow, 255 = lit), then blur + composite.
    mask = Image.new("L", (W, H), 255)
    draw = ImageDraw.Draw(mask)
    shade_val = int(DARK * 255)
    for c in cliffs:
        cx, cz = c["x"], c["z"] + DRESS_Z
        hw = c["w"] * TILE / 2.0 + FOOT_PAD
        hd = c["d"] * TILE / 2.0 + FOOT_PAD
        h = c["h"] * TILE
        ox, oz = SHADOW_DX_PER_H * h, SHADOW_DZ_PER_H * h
        # footprint corners + the same corners pushed along the shadow vector -> swept quad
        foot = [(cx - hw, cz - hd), (cx + hw, cz - hd), (cx + hw, cz + hd), (cx - hw, cz + hd)]
        cast = [(x + ox, z + oz) for (x, z) in foot]
        pts = foot + cast
        # convex hull (small point set) so the swept shadow is one solid blob
        pts = _hull(pts)
        poly = [world_to_px(x, z, W, H) for (x, z) in pts]
        draw.polygon(poly, fill=shade_val)
    mask = mask.filter(ImageFilter.GaussianBlur(BLUR_PX))

    # Composite: darken the base shade where the mask is dark. new = base * (mask/255 blended to DARK)
    out = Image.new("L", (W, H))
    bpx, mpx, opx = base.load(), mask.load(), out.load()
    for y in range(H):
        for x in range(W):
            f = mpx[x, y] / 255.0            # 1 lit .. DARK shadowed
            opx[x, y] = int(bpx[x, y] * f)
    out.convert("RGBA").save(OUT)
    print("wrote", os.path.relpath(OUT))


def _hull(points):
    pts = sorted(set(points))
    if len(pts) <= 2:
        return pts

    def cross(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    lower = []
    for p in pts:
        while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
            lower.pop()
        lower.append(p)
    upper = []
    for p in reversed(pts):
        while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
            upper.pop()
        upper.append(p)
    return lower[:-1] + upper[:-1]


if __name__ == "__main__":
    main()
