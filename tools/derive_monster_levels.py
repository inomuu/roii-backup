#!/usr/bin/env python3
"""derive_monster_levels.py — move monster strength off the ZONE and onto a LEVEL.

Today a monster's stats are `authored × its own multiplier × the ZONE's scale`, so the same
creature is a different creature depending on where it stands, and the difficulty curve is spread
across three places with nothing showing the total. Measuring it turned up two bands where the
game gets EASIER as you advance, which is the kind of defect that arrangement hides.

After this the model is one line:

    stat = level_base[zone.level] × monster.mult

`level_base` is a table, not a formula. Fitting one geometric rate to the real curve was measured
at -87% in the midgame, because today's curve is steep early and flat late; a table reproduces
what the game actually is.

    band -> level = floor(band × 5/3)      band 1->1, 3->5, 4->6, 60->100 (user 2026-08-07)

WHAT MOVES. The five gothic `scale_from_base` monsters (werewolf, maid, banshee, maid_horn,
mini_ghost) drift ±18% against their own band today, because each references one of five
placeholder mobs — `mob_b6_a`..`mob_b10_a`, which appear in no spawn pool and exist only to be
referenced. That drift is scaffolding showing through, not design, so it is CORRECTED rather than
preserved. Every other monster keeps its exact strength, within integer rounding.

This is why the script reports before it writes: the expected diff IS the acceptance test. A
monster outside the predicted list moving means the derivation is wrong.

Usage:  tools/derive_monster_levels.py            # report only
        tools/derive_monster_levels.py --write    # rewrite data/game_data.json
"""

import collections
import json
import os
import re
import statistics
import sys

DATA = "data/game_data.json"
MAX_LEVEL = 100


def band_of(zone_id):
    m = re.match(r"zone_b(\d+)_\d+$", zone_id)
    return int(m.group(1)) if m else None


def level_of(band):
    """One level per band. Derived from the three points the user gave: bands 1-3 span levels 1-5,
    bands 4-6 span 6-10, and band 4 is level 6."""
    return max(1, band * 5 // 3)


def effective(mon, zone, mons, stat):
    """Today's value: authored (or the band base's) × the monster's mult × the zone's scale."""
    base_id = mon.get("base_monster_id") or zone.get("base_monster_id", "")
    src = mons[base_id] if mon.get("scale_from_base") and base_id in mons else mon
    v = src.get(stat, 0)
    # XP has no scale of its own in any shipped zone — it rides hp_scale_bp, which is why
    # "this band is too hard" and "this band pays too little" were one edit.
    mult_key = {"xp_reward": "xp_mult_bp", "job_xp_reward": "job_xp_mult_bp"}.get(
        stat, f"{stat}_mult_bp")
    scale_key = "hp_scale_bp" if stat.endswith("_reward") else f"{stat}_scale_bp"
    v = v * mon.get(mult_key, 10000) / 10000
    return v * zone.get(scale_key, 10000) / 10000


def collect(data, stat):
    """{monster_id: {band: effective_stat}} across every band zone."""
    mons = {m["id"]: m for m in data["monsters"]}

    out = collections.defaultdict(dict)
    for z in data["zones"]:
        b = band_of(z["id"])
        if b is None:
            continue
        ids = [e["monster_id"] for e in z.get("spawn_pool", [])]
        if z.get("boss_monster_id"):
            ids.append(z["boss_monster_id"])
        for mid in ids:
            if mid not in mons:
                continue
            v = effective(mons[mid], z, mons, stat)
            if v:
                out[mid][b] = v
    return out


def derive(data, stat):
    """level_base[] plus one multiplier per monster.

    The band's reference is the GEOMETRIC MEAN of what stands in it. Arithmetic mean would let one
    high-HP boss drag the level's base above what the player actually meets; geometric mean is the
    typical opponent, which is what a difficulty curve should track.

    A monster's multiplier is the MEDIAN of its share across the bands it appears in — so a
    creature that drifts (the five gothic ones) lands on its typical share rather than on whichever
    band happened to be first.
    """
    eff = collect(data, stat)
    bands = sorted({b for r in eff.values() for b in r})
    col = {b: statistics.geometric_mean([r[b] for r in eff.values() if b in r]) for b in bands}
    mult = {}
    for mid, r in eff.items():
        mult[mid] = statistics.median(r[b] / col[b] for b in r)

    # Levels a band actually lands on; everything between is filled by geometric interpolation, so
    # a level nobody uses yet still has a sensible value the day a story encounter asks for it.
    at_level = {}
    for b in bands:
        at_level.setdefault(level_of(b), []).append(col[b])
    known = sorted(at_level)
    table = {}
    for lv in range(1, MAX_LEVEL + 1):
        if lv in at_level:
            table[lv] = statistics.geometric_mean(at_level[lv])
            continue
        lo = max((k for k in known if k < lv), default=None)
        hi = min((k for k in known if k > lv), default=None)
        if lo is None:
            table[lv] = statistics.geometric_mean(at_level[hi])
        elif hi is None:
            # Unreachable while the last band lands exactly on MAX_LEVEL, which it does
            # (band 60 -> level 100). Kept as an assertion rather than as extrapolation code that
            # cannot be exercised: past the table is the SIM's job, for Apex and cap raises.
            raise AssertionError("level %d is past the last band — extrapolation belongs in the sim" % lv)
        else:
            a = statistics.geometric_mean(at_level[lo])
            b2 = statistics.geometric_mean(at_level[hi])
            table[lv] = a * (b2 / a) ** ((lv - lo) / (hi - lo))
    return eff, col, mult, table


def orphans(data):
    """Monsters nothing can spawn: referenced ONLY as a zone's `base_monster_id`, which stops
    existing after this.

    Computed from what the zones actually reference, NOT from the `mob_b*` naming. A first version
    matched the name and deleted 32 monsters — 20 of which are real spawns — because the
    placeholders and the ordinary band mobs are named alike. Nothing caught it until the zones were
    re-checked afterwards, which is why this is reported in the DRY RUN now: a step that can delete
    data has to be visible before it runs, not only after.
    """
    spawned = set()
    for z in data["zones"]:
        for e in z.get("spawn_pool", []):
            spawned.add(e["monster_id"])
        if z.get("boss_monster_id"):
            spawned.add(z["boss_monster_id"])
    # BOTH conditions. Unreferenced alone would also take `marionette` — an authored monster that
    # simply is not placed in a zone yet. Deleting unused CONTENT is not this refactor's business;
    # deleting scaffolding is.
    return sorted(m["id"] for m in data["monsters"]
                  if m["id"] not in spawned and re.match(r"mob_b\d+_[a-z]$", m["id"]))


def main():
    write = "--write" in sys.argv
    data = json.load(open(DATA, encoding="utf-8"))

    report = {}
    for stat in ("hp", "atk", "xp_reward", "job_xp_reward"):
        eff, col, mult, table = derive(data, stat)
        report[stat] = (eff, col, mult, table)

    hp_eff, hp_col, hp_mult, hp_table = report["hp"]
    print("=" * 78)
    print("LEVEL BASE TABLE  (hp)")
    print("=" * 78)
    prev = None
    for lv in range(1, MAX_LEVEL + 1):
        if lv % 10 and lv != 1:
            continue
        r = "  x%.3f/lv" % ((hp_table[lv] / prev) ** (1 / 10)) if prev else ""
        print("  level %3d   base hp %9.0f%s" % (lv, hp_table[lv], r))
        prev = hp_table[lv]
    down = [lv for lv in range(2, MAX_LEVEL + 1) if hp_table[lv] < hp_table[lv - 1]]
    print("  levels where the table goes DOWN: %s" % (down or "none"))

    print()
    print("=" * 78)
    print("EFFECTIVE HP CHANGES  (anything not listed keeps its exact value)")
    print("=" * 78)
    moved = []
    for mid, r in hp_eff.items():
        for b, now in r.items():
            after = hp_table[level_of(b)] * hp_mult[mid]
            if abs(after / now - 1) > 0.005:
                moved.append((abs(after / now - 1), mid, b, now, after))
    moved.sort(reverse=True)
    for _, mid, b, now, after in moved:
        print("  %-18s band %2d   %8.0f -> %8.0f   %+6.1f%%"
              % (mid, b, now, after, (after / now - 1) * 100))
    print("  %d of %d placements move, %d distinct monsters"
          % (len(moved), sum(len(r) for r in hp_eff.values()), len({m[1] for m in moved})))

    dead = orphans(data)
    print("\nWILL DELETE %d monsters that no zone can spawn (referenced only as a band base): %s"
          % (len(dead), dead))

    if not write:
        print("\n(report only — pass --write to rewrite %s)" % DATA)
        return

    mons = {m["id"]: m for m in data["monsters"]}

    # ZONE LEVELS FIRST, before a single monster is touched.
    #
    # `solve_level` reads today's effective stats, and those are only readable while the OLD shape
    # is intact: the moment the loop below overwrites the multipliers and drops `scale_from_base`, a
    # gothic mob reads as its own authored `hp: 1` instead of the band base it derives from, every
    # reference collapses to ~1, and every solved zone lands on level 1. It did.
    # Every zone needs a level, band or not: the dungeons and the story stages are zones too, and a
    # zone without one has no base to look up. A band zone takes its level from the band; anything
    # else is SOLVED — find the level whose base best reproduces what that zone's monsters already
    # hit for, so a Duskmourn room stays exactly as hard as it is today.
    def solve_level(z):
        """The level whose row best reproduces what this zone's monsters already hit for.

        Scored across HP **and** ATK together, in relative terms. Matching HP alone picked a level
        whose attack column was 24% adrift for the Duskmourn bosses: the two curves were derived
        independently, so the level that fits one need not fit the other. Relative error, because an
        absolute one would let HP in the thousands drown out ATK in the hundreds.
        """
        refs = {}
        bm = z.get("base_monster_id", "")
        for stat, table in (("hp", hp_table), ("atk", report["atk"][3])):
            mult = report[stat][2]
            pool = [effective(mons[e["monster_id"]], z, mons, stat)
                    for e in z.get("spawn_pool", []) if e["monster_id"] in mons]
            pool = [v for v in pool if v]
            shares = [mult[e["monster_id"]] for e in z.get("spawn_pool", [])
                      if e["monster_id"] in mult]
            if pool and shares:
                refs[stat] = (statistics.geometric_mean(pool)
                              / statistics.geometric_mean(shares), table)
            elif bm in mons:
                # Only when the room's own pool cannot answer. Reading the shared
                # `base_monster_id` FIRST gave all four Duskmourn rooms the same level, because they
                # share one — flattening a dungeon whose whole shape is that the rooms escalate.
                refs[stat] = (mons[bm].get(stat, 0)
                              * z.get(f"{stat}_scale_bp", 10000) / 10000, table)
        refs = {k: v for k, v in refs.items() if v[0] > 0}
        if not refs:
            return 1
        def err(lv):
            return sum(abs(table[lv] / ref - 1) for ref, table in refs.values())
        return min(range(1, MAX_LEVEL + 1), key=err)

    for z in data["zones"]:
        b = band_of(z["id"])
        z["level"] = level_of(b) if b is not None else solve_level(z)

    # PASS 3 — multipliers for monsters that live ONLY outside the bands.
    #
    # `collect` reads band zones only, because those are the ones whose level is known up front. A
    # dungeon-only monster therefore never gets a new multiplier, keeps the OLD one — which meant a
    # share of a base that no longer exists — and gets it applied to the new level row. Duskmourn's
    # wardens came out 24% weak that way. Now that every zone HAS a level, they can be solved
    # against their own room.
    for stat in ("hp", "atk", "xp_reward", "job_xp_reward"):
        mult = report[stat][2]
        table = report[stat][3]
        for z in data["zones"]:
            lv = z.get("level", 0)
            if not lv or table.get(lv, 0) <= 0:
                continue
            ids = [e["monster_id"] for e in z.get("spawn_pool", [])]
            if z.get("boss_monster_id"):
                ids.append(z["boss_monster_id"])
            for mid in ids:
                if mid in mult or mid not in mons:
                    continue
                was = effective(mons[mid], z, mons, stat)
                if was:
                    mult[mid] = was / table[lv]


    # BAKE what a scale_from_base mob inherits but the new model does not carry. def / mdef / hit /
    # flee are NOT zone-scaled, so they never belonged to the level curve — they come straight off
    # the placeholder base. The five gothic mobs each have `def: 0` and no mdef/hit/flee at all, so
    # deleting the flag without this leaves them unable to hit anything and unable to take a hit:
    # the whole 16-30 arc, silently. Median across the bands they appear in, same as the mults.
    plain = ("def", "mdef", "hit", "flee")
    baked = {}
    for z in data["zones"]:
        b = band_of(z["id"])
        if b is None or not z.get("base_monster_id"):
            continue
        base = mons.get(z["base_monster_id"])
        if base is None:
            continue
        ids = [e["monster_id"] for e in z.get("spawn_pool", [])]
        if z.get("boss_monster_id"):
            ids.append(z["boss_monster_id"])
        for mid in ids:
            m = mons.get(mid)
            if m is None or not m.get("scale_from_base"):
                continue
            for k in plain:
                v = base.get(k, 0) * m.get(f"{k}_mult_bp", 10000) / 10000
                baked.setdefault(mid, {}).setdefault(k, []).append(v)
            baked[mid].setdefault("_level", []).append(level_of(b))
            baked[mid].setdefault("gil_reward", []).append(base.get("gil_reward", 0))
            baked[mid].setdefault("level", []).append(
                base.get("level", 1) + m.get("level_add", 0))
    # hit and flee come from the FORMULA, not from the median of what was inherited. It is
    # documented (0.0.111: mob hit/flee are the pre-renewal `Lv + DEX` / `Lv + AGI` port plus the
    # renewal bases HIT_BASE 175 / FLEE_BASE 100) and fitting it over the 48 monsters that were
    # actually converted gives a median residual of 2.5. The two methods agree within 3 points on
    # all five mobs, which is the cross-check; the formula wins because it extends to any level a
    # story encounter or Apex asks for, instead of five baked constants.
    hit_a, hit_b = 185.1, 1.339
    flee_a, flee_b = 99.2, 1.315
    for mid, vals in baked.items():
        lv = statistics.median(vals.pop("_level"))
        for k, vs in vals.items():
            mons[mid][k] = max(0, round(statistics.median(vs)))
        mons[mid]["hit"] = round(hit_a + hit_b * lv)
        mons[mid]["flee"] = round(flee_a + flee_b * lv)
        for k in plain:
            mons[mid].pop(f"{k}_mult_bp", None)
        mons[mid].pop("level_add", None)
    print("\nbaked def/mdef/hit/flee/gil/level into %d scale_from_base monsters: %s"
          % (len(baked), sorted(baked)))

    data["level_base"] = [
        {"level": lv,
         "hp": max(1, round(hp_table[lv])),
         "atk": max(1, round(report["atk"][3][lv])),
         "xp": max(1, round(report["xp_reward"][3][lv])),
         "job_xp": max(1, round(report["job_xp_reward"][3][lv]))}
        for lv in range(1, MAX_LEVEL + 1)
    ]
    for m in data["monsters"]:
        if m["id"] in hp_mult:
            m["hp_mult_bp"] = max(1, round(hp_mult[m["id"]] * 10000))
        for stat, key in (("atk", "atk_mult_bp"), ("xp_reward", "xp_mult_bp"),
                          ("job_xp_reward", "job_xp_mult_bp")):
            if m["id"] in report[stat][2]:
                m[key] = max(1, round(report[stat][2][m["id"]] * 10000))
        for dead in ("scale_from_base", "base_monster_id"):
            m.pop(dead, None)
        for dead in ("hp_scale_bp", "atk_scale_bp", "xp_scale_bp", "base_monster_id"):
            z.pop(dead, None)
    data["monsters"] = [m for m in data["monsters"] if m["id"] not in orphans(data)]
    print("\nremoved %d reference-only monsters: %s" % (len(orphans(data)), orphans(data)))

    tmp = DATA + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=1)
    os.replace(tmp, DATA)
    print("wrote %s" % DATA)


if __name__ == "__main__":
    main()
