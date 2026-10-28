#!/usr/bin/env python3
"""Build a zone background: Tiled export + hand-painted layers -> graded PNG the game loads.

    tools/build_bg.py 1                 # art/bg_src/{tiles,paint*}_r1.png -> godot/assets/bg/bg_r1.png
    tools/build_bg.py 1 --raw           # skip the grade (see what Tiled actually gave you)
    tools/build_bg.py 1 --hue 0.2       # try a value without touching render_data.gd

PAINT ON THE UNGRADED TILES, not on a graded export. The grade stays one number you can still
turn; bake it into hand work and it becomes a colour you have to repaint. Keep a graded preview
open beside you while painting and match against that.

The grade is the EXACT maths of zone_bg.gdshader: hue rotates about the grey axis (Rodrigues
about 1/sqrt(3)), which is NOT what "hue +59 degrees" does in Photoshop or Aseprite. Doing it by
eye in an editor quietly shifts the tone away from the one that was chosen in game. Verified
equal to the shader's cross-product form to 6e-9 over 20k random colours.

Cross-machine: Tiled and painting happen wherever they are comfortable (Tiled is awkward on
macOS), this runs where the repo is. The handoff is just the files in art/bg_src/ — see the
README there.
"""
import argparse, math, os, sys

# render_data.gd. While ANY background is still shipped ungraded the Arena shader must stay on,
# so these must match it — flip both to identity only when every bg_r*.png here is baked.
# Identity. 0.165 / 0.89 / 1.05 was tuned against a STOCK placeholder and does not transfer:
# 0.165 turns is a 59-degree rotation that turns the hand-made art's grass cyan. A grade belongs
# to one art set; pass the numbers on the command line rather than inheriting someone else's.
HUE, SAT, BRIGHT = 0.0, 1.0, 1.0
# 34 x 15 tiles at 32 px. DERIVED, not chosen: the arena always fits its 24x12 grid into the
# visible area (`cell_px = floor(min(avail_w/24, avail_h/12))`), so one 32 px art tile is a fixed
# fraction of a cell and the tile count depends on the window's ASPECT, not its size. Height is
# always 15; width is aspect x 15, which is 34 at the 1600x900 default. A larger window at the
# same aspect needs FEWER tiles, because the fixed-pixel HUD eats a smaller share of the height.
CANVAS = (1088, 480)
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def _pillow():
    """Homebrew's python3.14 is broken here (pyexpat symbol) and cannot even run pip, while the
    system python already has Pillow. Re-exec rather than make every caller remember which."""
    try:
        from PIL import Image
        return Image
    except ImportError:
        fallback = "/usr/bin/python3"
        if os.path.exists(fallback) and os.path.realpath(sys.executable) != os.path.realpath(fallback) \
                and not os.environ.get("_BG_REEXEC"):
            os.environ["_BG_REEXEC"] = "1"
            os.execv(fallback, [fallback] + sys.argv)
        sys.exit("needs Pillow:  /usr/bin/python3 -m pip install --user Pillow")


def grade(im, hue, sat, bright):
    px = im.load()
    a = hue * 2.0 * math.pi
    k = 1.0 / math.sqrt(3.0)
    c, s = math.cos(a), math.sin(a)
    m = [[c + k*k*(1-c), k*k*(1-c) - k*s, k*k*(1-c) + k*s],
         [k*k*(1-c) + k*s, c + k*k*(1-c), k*k*(1-c) - k*s],
         [k*k*(1-c) - k*s, k*k*(1-c) + k*s, c + k*k*(1-c)]]
    lum = (0.2126, 0.7152, 0.0722)   # Rec. 709 — pulling colour out must not also darken
    for y in range(im.height):
        for x in range(im.width):
            r, g, b, al = px[x, y]
            v = [r / 255.0, g / 255.0, b / 255.0]
            if hue:
                v = [min(1.0, max(0.0, sum(m[i][j] * v[j] for j in range(3)))) for i in range(3)]
            l = sum(lum[i] * v[i] for i in range(3))
            v = [min(1.0, max(0.0, (l + (v[i] - l) * sat) * bright)) for i in range(3)]
            px[x, y] = (round(v[0] * 255), round(v[1] * 255), round(v[2] * 255), al)
    return im


def main():
    p = argparse.ArgumentParser(description="Build a zone background from its source layers.")
    p.add_argument("region", type=int, help="the number the GAME SHOWS (zone '3-1' is region 3)")
    p.add_argument("--src", default=os.path.join(ROOT, "art", "bg_src"))
    p.add_argument("--out", default=None)
    p.add_argument("--raw", action="store_true", help="skip the grade")
    p.add_argument("--hue", type=float, default=HUE)
    p.add_argument("--sat", type=float, default=SAT)
    p.add_argument("--bright", type=float, default=BRIGHT)
    a = p.parse_args()
    Image = _pillow()

    tiles = os.path.join(a.src, "tiles_r%d.png" % a.region)
    if not os.path.exists(tiles):
        sys.exit("no %s — export the Tiled map there first (see %s/README.md)" % (tiles, a.src))
    base = Image.open(tiles).convert("RGBA")
    if base.width < CANVAS[0] or base.height < CANVAS[1]:
        # Only UNDERSIZE is a problem. The renderer centres the texture without tiling, so short
        # art leaves bare grid at the edges while oversized art simply extends past the play area
        # (the HUD is on its own canvas layers and is not covered). Said here, where it is cheap
        # to fix, rather than in-engine at a band nobody happens to be standing in.
        print("WARNING: %dx%d is smaller than %dx%d (%d x %d tiles at 32 px) — the edges of the "
              "view will show bare grid. Oversized is free; undersized is not."
              % (base.width, base.height, CANVAS[0], CANVAS[1], CANVAS[0] // 32, CANVAS[1] // 32))

    layers = sorted(f for f in os.listdir(a.src)
                    if f.startswith("paint") and f.endswith("_r%d.png" % a.region))
    for name in layers:
        over = Image.open(os.path.join(a.src, name)).convert("RGBA")
        if over.size != base.size:
            sys.exit("%s is %dx%d but the tiles are %dx%d — paint on a copy of the export"
                     % (name, over.width, over.height, base.width, base.height))
        base = Image.alpha_composite(base, over)

    if not a.raw:
        base = grade(base, a.hue, a.sat, a.bright)

    out = a.out or os.path.join(ROOT, "godot", "assets", "bg", "bg_r%d.png" % a.region)
    base.save(out)
    print("region %d: tiles + %d paint layer(s)%s -> %s"
          % (a.region, len(layers),
             "" if a.raw else " + grade(hue %.3f sat %.2f bright %.2f)" % (a.hue, a.sat, a.bright),
             os.path.relpath(out, ROOT)))
    if layers:
        print("  painted: " + ", ".join(layers))


main()
