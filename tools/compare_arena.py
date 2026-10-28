#!/usr/bin/env python3
"""Diff the Godot and Unity arena dumps ROW BY ROW.

    tools/compare_arena.py /tmp/godot_arena.csv /tmp/unity_arena.csv

Both files are written by the env-gated dump pass in each engine (proto/arena3d/arena_dumper.gd,
Assets/Editor/ArenaDump.cs) from the SAME seed + zone + class, so a mismatch is a port defect,
not noise. Position tolerance is a fraction of a cell because the two engines sample their own
frame clocks — anything the eye could catch is far outside it.
"""
import csv, sys
from collections import defaultdict

POS_TOL = 0.12          # cells; one glide frame at 10 Hz moves ~0.1
# Clips whose frame phase is FREE-RUNNING off each engine's own clock (nothing in the sim
# anchors them), so their frame index carries no information and comparing it would report a
# difference forever. Everything else — attack, hurt, death — is event-anchored and IS compared.
FREE_RUNNING = {"idle", "move"}


def clip_tables(tables_path):
    """row_N (Godot's SpriteFrames naming) -> anim name, plus the monster sheet-frame lists.
    Read from the GENERATED arena_tables.json so this never re-encodes the art layout itself."""
    import json
    t = json.load(open(tables_path))
    rows = {}
    for c in t["classes"]:
        for name, key in (("idle", "idle_row"), ("walk", "walk_row"), ("attack", "atk_row"), ("death", "death_row")):
            rows.setdefault("row_%d" % c[key], name)
    mon = t["monster"]
    frames = {"idle": mon["idle_frames"], "move": mon["move_frames"], "attack": mon["atk_frames"],
              "hurt": mon["damaged_frames"], "death": mon["die_frames"]}
    return rows, frames


def load(path):
    rows = defaultdict(dict)
    with open(path) as f:
        for r in csv.DictReader(f):
            rows[int(r["tick"])][int(r["id"])] = r
    return rows

def norm(v):
    return ALIAS.get(v, v)

def main(a_path, b_path, tables="unity/ROI/Assets/StreamingAssets/art/arena_tables.json"):
    rows, mon_frames = clip_tables(tables)
    a, b = load(a_path), load(b_path)
    ticks = sorted(set(a) & set(b))
    if not ticks:
        print("no common ticks — did both runs use the same ARENA_DUMP_TICKS?")
        return 1

    def clip(r):
        c = rows.get(r["clip"], r["clip"])        # player: row_N -> anim name
        return {"damaged": "hurt", "walk": "move"}.get(c, c)

    def frame_in_clip(r, name, kind):
        """Godot reports the index WITHIN the clip; Unity reports the sheet frame for monsters.
        Normalise to the clip index so the two are the same quantity."""
        f = int(r["frame"])
        if kind == "monster" and name in mon_frames and f in mon_frames[name]:
            return mon_frames[name].index(f)
        return f

    bad = 0
    for t in ticks:
        for i in sorted(set(a[t]) | set(b[t])):
            ra, rb = a[t].get(i), b[t].get(i)
            if ra is None or rb is None:
                print(f"tick {t} id {i}: present only in {'unity' if ra is None else 'godot'}")
                bad += 1
                continue
            # id -1/-2 are the SCENE rows (ground, camera): geometry, not actors. They carry
            # size/placement in the position + px_size/alpha columns and have no clip or frame.
            if i < 0:
                for k, label in (("cell_x", "x"), ("cell_y", "z"),
                                 ("px_size", "size_x/fov"), ("alpha", "size_z/height")):
                    d = abs(float(ra[k]) - float(rb[k]))
                    if d > 0.01:
                        print(f"tick {t} {ra['kind']}: {label} godot={ra[k]} unity={rb[k]}")
                        bad += 1
                # Camera yaw differs by 180 BY DESIGN: this scene mirrors world x and turns the
                # camera around, so screen-space matches. Compare it modulo that convention.
                flip = 180 if ra["kind"] == "camera" else 0
                yaw_a, yaw_b = int(ra["facing"]) % 360, (int(rb["facing"]) + flip) % 360
                if ra["frame"] != rb["frame"] or yaw_a != yaw_b:
                    print(f"tick {t} {ra['kind']}: angles godot=(pitch {ra['frame']}, yaw {ra['facing']}) "
                          f"unity=(pitch {rb['frame']}, yaw {rb['facing']})")
                    bad += 1
                continue
            kind = "player" if "player" in ra["kind"] else "monster"
            for k in ("cell_x", "cell_y"):
                d = abs(float(ra[k]) - float(rb[k]))
                if d > POS_TOL:
                    print(f"tick {t} id {i} ({kind}): {k} godot={ra[k]} unity={rb[k]} (delta {d:.3f})")
                    bad += 1
            ca, cb = clip(ra), clip(rb)
            if ca != cb:
                print(f"tick {t} id {i} ({kind}): clip godot={ca} unity={cb}")
                bad += 1
            elif ca not in FREE_RUNNING:
                fa, fb = frame_in_clip(ra, ca, kind), frame_in_clip(rb, cb, kind)
                if fa != fb:
                    print(f"tick {t} id {i} ({kind}): {ca} frame godot={fa} unity={fb}")
                    bad += 1
            if ra["facing"] != rb["facing"]:
                print(f"tick {t} id {i} ({kind}): facing godot={ra['facing']} unity={rb['facing']}")
                bad += 1
            if abs(float(ra["px_size"]) - float(rb["px_size"])) > 1e-4:
                print(f"tick {t} id {i} ({kind}): px_size godot={ra['px_size']} unity={rb['px_size']}")
                bad += 1
    n = sum(len(a[t]) for t in ticks)
    print(f"\n{len(ticks)} ticks, {n} godot rows compared — {bad} mismatch(es)")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(*(sys.argv[1:3] or ["/tmp/godot_arena.csv", "/tmp/unity_arena.csv"])))
