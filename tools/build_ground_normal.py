#!/usr/bin/env python3
"""Generate a tangent-space normal map for a zone background: bg_r<n>.png -> bg_r<n>_n.png.

    tools/build_ground_normal.py 1 [--fine 1.5] [--broad 0.8] [--broad-radius 10]

WHY OFFLINE, when ground_recolour.gdshader already fakes relief from the albedo in-shader
(`bump`). The shader can only see one pixel and its two neighbours, so its grooves are one art
pixel wide — edges, not shapes. Offline there is room to build a HEIGHT field at two scales
(fine detail straight from luminance, broad mounds from a box-blurred copy) and take the
gradient of that, which is what turns a dark joint line into a bevel with two lit sides.

Same trade as the shader and worth restating: LUMINANCE IS NOT HEIGHT. A painted-dark stone
reads as a dip whether or not it is one. It survives at this camera distance; do not expect it
to survive a close-up.

Pure stdlib, like apply_palette.py (this machine has no numpy/PIL), and it REUSES that file's
PNG codec — one copy of the unfilter table, not two drifting ones.

The normal's sign convention matches the `bump` branch in ground_recolour.gdshader
((l - l_right, l - l_down, 1) -> RGB), which was validated by eye in-game. If stones ever look
inverted (lit from the wrong side), the fix belongs HERE, in one place.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from apply_palette import read_png, unfilter, chunk
import struct, zlib


def box_blur(src, w, h, r):
    """Two-pass running-sum box blur on a flat float list. O(n) per pass, radius-independent."""
    if r <= 0:
        return src[:]
    tmp = [0.0] * (w * h)
    inv = 1.0 / (2 * r + 1)
    for y in range(h):
        row = y * w
        acc = src[row] * (r + 1)
        for x in range(1, r + 1):
            acc += src[row + min(x, w - 1)]
        for x in range(w):
            tmp[row + x] = acc * inv
            acc += src[row + min(x + r + 1, w - 1)] - src[row + max(x - r, 0)]
    out = [0.0] * (w * h)
    for x in range(w):
        acc = tmp[x] * (r + 1)
        for y in range(1, r + 1):
            acc += tmp[min(y, h - 1) * w + x]
        for y in range(h):
            out[y * w + x] = acc * inv
            acc += tmp[min(y + r + 1, h - 1) * w + x] - tmp[max(y - r, 0) * w + x]
    return out


def main():
    if len(sys.argv) < 2 or not sys.argv[1].isdigit():
        sys.exit(__doc__)
    region = int(sys.argv[1])
    args = sys.argv[2:]

    def opt(name, default):
        return float(args[args.index(name) + 1]) if name in args else default

    fine = opt("--fine", 1.5)
    # 0.25, down from the 0.8 this shipped with: the broad layer's wide gradients painted a
    # smooth shading copy of the art displaced by the blur radius, which on screen read as a
    # GHOST IMAGE (user, 2026-08-24: "เห็นเป็นภาพซ้อนๆ"). A trace of it still helps the road
    # crown; most of the relief belongs to the fine layer, which stays aligned to the pixel.
    broad = opt("--broad", 0.25)
    broad_r = int(opt("--broad-radius", 6))

    src = Path(__file__).parent.parent / "godot" / "assets" / "bg" / f"bg_r{region}.png"
    dst = src.with_name(f"bg_r{region}_n.png")
    png = read_png(src)
    if png is None:
        sys.exit(f"cannot read {src} (need 8-bit non-interlaced)")
    w, h, ct = png["w"], png["h"], png["ct"]
    ch = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[ct]
    px = unfilter(zlib.decompress(png["idat"]), w, h, ch)

    # Luminance 0..1. Paletted images go through the palette; everything else reads RGB direct.
    lum = [0.0] * (w * h)
    pal = png["pal"]
    for i in range(w * h):
        o = i * ch
        if ct == 3:
            p = px[o] * 3
            r, g, b = pal[p], pal[p + 1], pal[p + 2]
        else:
            r, g, b = px[o], px[o + 1], px[o + 2]
        lum[i] = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0

    # The fine layer differences RAW luminance — no pre-blur. A radius-1 smooth was here at
    # first "to kill dithering", and it was half of why the ground read as blurry: soften the
    # height and every facet softens with it. The in-shader bump always differenced raw pixels
    # and was the crisp one. Dither noise turns out to be what pixel-art grain looks like.
    mound = box_blur(lum, w, h, broad_r)     # the broad shapes: crown of the road, grass mats
    height = [lum[i] * fine + mound[i] * broad for i in range(w * h)]

    out = bytearray(w * h * 3)
    for y in range(h):
        row = y * w
        for x in range(w):
            i = row + x
            # CENTERED differences, not forward: h(x)-h(x+1) puts every slope half a pixel
            # to the right of the edge that caused it, so the whole relief sat displaced from
            # the art — the other half of the ghost-image report.
            dx = (height[row + max(x - 1, 0)] - height[row + min(x + 1, w - 1)]) * 0.5
            dy = (height[max(y - 1, 0) * w + x] - height[min(y + 1, h - 1) * w + x]) * 0.5
            # normalize (dx, dy, z) with z chosen so slopes stay in range without clipping
            nx, ny, nz = dx * 8.0, dy * 8.0, 1.0
            ln = (nx * nx + ny * ny + nz * nz) ** 0.5
            o = i * 3
            out[o] = min(255, max(0, int((nx / ln * 0.5 + 0.5) * 255)))
            out[o + 1] = min(255, max(0, int((ny / ln * 0.5 + 0.5) * 255)))
            out[o + 2] = min(255, max(0, int((nz / ln * 0.5 + 0.5) * 255)))

    body = b"\x89PNG\r\n\x1a\n"
    body += chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
    raw = bytearray()
    for y in range(h):
        raw += b"\x00" + out[y * w * 3:(y + 1) * w * 3]
    body += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    body += chunk(b"IEND", b"")
    dst.write_bytes(body)
    print(f"{dst}  {w}x{h}  fine={fine} broad={broad} r={broad_r}")


if __name__ == "__main__":
    main()
