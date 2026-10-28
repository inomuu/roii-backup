#!/usr/bin/env python3
"""Recolour a tileset by PER-COLOUR mapping, from tools/palette_map.csv.

Why not an HSL adjustment. Measured on the two reference mocks: a single hue/saturation curve
bent the greens by +20 to +57 degrees depending where each sat in the range, so grass and foliage
that were 9 degrees apart came out 35 apart. Worse, dirt (H28) lost half its saturation while the
tan rocks beside it (H31) nearly doubled theirs — two materials a few degrees apart moving in
opposite directions. A curve cannot tell them apart; a table can, because you write the
destination yourself.

    tools/apply_palette.py <src-dir> <out-dir> [--map tools/palette_map.csv] [--dry-run]

Every colour NOT in the table is passed through unchanged and reported at the end, so a tileset
that uses colours the table has never seen fails loudly instead of coming out half-graded.

Paletted PNGs are recoloured by rewriting PLTE alone — the pixel data is untouched, so the file
stays byte-for-byte the same size and no resampling can creep in. Truecolour PNGs are decoded and
re-encoded; alpha is preserved exactly.
"""
import csv
import struct
import sys
import zlib
from collections import Counter
from pathlib import Path


def read_png(path):
    d = path.read_bytes()
    if d[:8] != b"\x89PNG\r\n\x1a\n":
        return None
    pos, chunks, idat = 8, [], b""
    w = h = bd = ct = None
    pal = None
    while pos < len(d):
        ln = struct.unpack(">I", d[pos:pos + 4])[0]
        typ = d[pos + 4:pos + 8]
        data = d[pos + 8:pos + 8 + ln]
        pos += 12 + ln
        if typ == b"IHDR":
            w, h, bd, ct = struct.unpack(">IIBB", data[:10])
            interlace = data[12]
        elif typ == b"PLTE":
            pal = bytearray(data)
        elif typ == b"IDAT":
            idat += data
            continue
        chunks.append((typ, data))
    if bd != 8 or interlace != 0:
        return None
    return dict(w=w, h=h, bd=bd, ct=ct, pal=pal, idat=idat, chunks=chunks)


def unfilter(raw, w, h, ch):
    stride = w * ch
    out = bytearray()
    prev = bytearray(stride)
    i = 0
    for _ in range(h):
        f = raw[i]
        i += 1
        line = bytearray(raw[i:i + stride])
        i += stride
        for x in range(stride):
            a = line[x - ch] if x >= ch else 0
            b = prev[x]
            c = prev[x - ch] if x >= ch else 0
            if f == 1:
                line[x] = (line[x] + a) & 255
            elif f == 2:
                line[x] = (line[x] + b) & 255
            elif f == 3:
                line[x] = (line[x] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[x] = (line[x] + pr) & 255
        out += line
        prev = line
    return out


def chunk(typ, data):
    return struct.pack(">I", len(data)) + typ + data + struct.pack(">I", zlib.crc32(typ + data))


def write_png(path, png, pixels=None, pal=None):
    body = b"\x89PNG\r\n\x1a\n"
    stride = png["w"] * {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[png["ct"]]
    for typ, data in png["chunks"]:
        if typ == b"PLTE" and pal is not None:
            data = bytes(pal)
        body += chunk(typ, data)
        if typ == b"IHDR":
            if pixels is not None:
                raw = bytearray()
                for y in range(png["h"]):
                    raw += b"\x00" + pixels[y * stride:(y + 1) * stride]
                body += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
            else:
                body += chunk(b"IDAT", png["idat"])
    path.write_bytes(body)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    flags = {a for a in sys.argv[1:] if a.startswith("--")}
    if len(args) < 2:
        print(__doc__)
        return 2
    src_dir, out_dir = Path(args[0]), Path(args[1])
    map_path = Path("tools/palette_map.csv")
    for a in sys.argv[1:]:
        if a.startswith("--map="):
            map_path = Path(a.split("=", 1)[1])

    table = {}
    for r in csv.DictReader(open(map_path)):
        s = tuple(int(r["src"][i:i + 2], 16) for i in (1, 3, 5))
        d = tuple(int(r["dst"][i:i + 2], 16) for i in (1, 3, 5))
        table[s] = d
    print("%d mappings from %s" % (len(table), map_path))

    unmapped = Counter()
    done = skipped = 0
    for f in sorted(src_dir.rglob("*.png")):
        png = read_png(f)
        rel = f.relative_to(src_dir)
        if png is None:
            skipped += 1
            continue
        ch = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[png["ct"]]
        dst = out_dir / rel
        if png["ct"] == 3 and png["pal"] is not None:
            # Palette-only rewrite: the image data never moves.
            pal = bytearray(png["pal"])
            for i in range(0, len(pal), 3):
                c = (pal[i], pal[i + 1], pal[i + 2])
                if c in table:
                    pal[i], pal[i + 1], pal[i + 2] = table[c]
                else:
                    unmapped[c] += 1
            if "--dry-run" not in flags:
                dst.parent.mkdir(parents=True, exist_ok=True)
                write_png(dst, png, pal=pal)
        else:
            px = unfilter(zlib.decompress(png["idat"]), png["w"], png["h"], ch)
            for i in range(0, len(px), ch):
                if png["ct"] == 6 and px[i + 3] == 0:
                    continue  # fully transparent: colour is meaningless, leave it
                c = (px[i], px[i + 1], px[i + 2])
                if c in table:
                    px[i], px[i + 1], px[i + 2] = table[c]
                else:
                    unmapped[c] += 1
            if "--dry-run" not in flags:
                dst.parent.mkdir(parents=True, exist_ok=True)
                write_png(dst, png, pixels=px)
        done += 1

    print("recoloured %d png (%d skipped: interlaced or not 8-bit)" % (done, skipped))
    if unmapped:
        total = sum(unmapped.values())
        print("\n%d colours had NO mapping (%d pixels) — passed through unchanged:" % (len(unmapped), total))
        for c, k in unmapped.most_common(20):
            print("   #%02X%02X%02X  %9d px" % (c + (k,)))
        if len(unmapped) > 20:
            print("   … and %d more" % (len(unmapped) - 20))
    else:
        print("every colour in the source was covered by the table")
    return 0


if __name__ == "__main__":
    sys.exit(main())
