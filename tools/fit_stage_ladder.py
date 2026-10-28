#!/usr/bin/env python3
"""fit_stage_ladder.py — fit `stage_base` to the party's MEASURED power so the ladder is climbed at
the target pace instead of walling wherever a fixed slope outruns the party.

The idea: a sim-cli run records the highest stage the party has beaten at every hour under the
CURRENT table. That is a measurement of "what difficulty the party can handle at hour h":
`can(h) = current_row[stage_reached(h)]`. The target says which stage the party SHOULD be on at
hour h, so the new row for that stage is simply the difficulty the party could handle then:

    new_row[target_stage(h)] = current_row[stage_reached(h)]

Stages past the last measured hour continue on the slope of the last fitted decade (the run
stalls before the cap; the next iteration measures further). hp / atk / xp all move together
(a stage is one difficulty), def / toughness stay on their stage index.

    tools/fit_stage_ladder.py run.csv            # report only
    tools/fit_stage_ladder.py run.csv --write    # rewrite data/game_data.json (bump the version yourself)

`run.csv` is `sim-cli --sample 36000 --out run.csv` (hourly rows: tick, zone, …).
"""
import csv, json, math, re, sys

# TARGET: stage reached at the end of each sim day (24 h), design §4 — ~30/day early, ~10/day late,
# the launch cap (168) around day 12.
TARGET_BY_DAY = [0, 30, 55, 75, 92, 106, 118, 129, 139, 148, 156, 162, 168]
STAGES = 240
FIT_MIN_HOURS = 6   # ignore the first hours (the run is still recruiting)
STALL_HOURS = 12    # the max stage unchanged this long = a wall, not a pace


def target_stage(hour):
    d = hour / 24.0
    if d >= len(TARGET_BY_DAY) - 1:
        return TARGET_BY_DAY[-1]
    lo = int(d)
    f = d - lo
    return TARGET_BY_DAY[lo] + (TARGET_BY_DAY[lo + 1] - TARGET_BY_DAY[lo]) * f


def fit_power(rows, data, write):
    """`--power`: fit the table to the party's POWER instead of to the stage it reached.

    A reached stage says only that the party could grind past it (the auto-player dies and
    retries); the party's summed effective ATK and max HP say what it can actually fight. Room
    difficulty is set so that, at the hour the target puts the party on stage s, the ROOM's total
    HP is a fixed multiple of the party's ATK and the room's total ATK a fixed multiple of the
    party's HP — the multiples calibrated on stage 1, where the game is known to be easy:

        mob_hp(s)  = C_HP  · party_atk(h*(s)) / pop(s)
        mob_atk(s) = C_ATK · party_hp(h*(s))  / pop(s)

    `pop(s)` is the zone's `target_population` (2 early, 7 late), so more monsters in a room means
    each is weaker. XP per kill stays in proportion to the monster's HP (stage 1's ratio).
    """
    zones = {z["stage"]: z for z in data["zones"] if z["id"].endswith("_1") and z["id"].startswith("zone_b")}
    pop = lambda s: max(1, zones.get(s, zones[1]).get("target_population", 2))
    cur = {r["stage"]: r for r in data["stage_base"]}
    power = {}
    for r in rows:
        h = int(r["tick"]) / 36000.0
        power[round(h)] = (int(r["party_atk"]), int(r["party_hp"]))
    hours = sorted(power)
    # Calibrate on stage 1 against the party at hour 0 (one member, starter gear).
    a0, h0 = power[hours[0]]
    c_hp = cur[1]["hp"] * pop(1) / max(1, a0)
    c_atk = cur[1]["atk"] * pop(1) / max(1, h0)
    c_hp *= POWER_MARGIN
    c_atk *= POWER_MARGIN
    xp_ratio = cur[1]["xp"] / max(1, cur[1]["hp"])
    print(f"calibration: party at h{hours[0]} atk {a0} hp {h0} → C_HP {c_hp:.3f} C_ATK {c_atk:.4f} (margin ×{POWER_MARGIN})")

    def hour_of_stage(s):
        # invert the target: the hour the party should first stand on stage s
        for h in range(0, 24 * (len(TARGET_BY_DAY) - 1) + 1):
            if target_stage(h) >= s:
                return h
        return None

    def party_at(h):
        # linear interpolation of the hourly samples; past the run, extrapolate on the last day's slope
        hs = hours
        if h <= hs[0]:
            return power[hs[0]]
        if h >= hs[-1]:
            (a1, p1), (a2, p2) = power[hs[-25] if len(hs) > 25 else hs[0]], power[hs[-1]]
            span = hs[-1] - (hs[-25] if len(hs) > 25 else hs[0])
            k = (h - hs[-1]) / max(1, span)
            return (a2 * (a2 / max(1, a1)) ** k, p2 * (p2 / max(1, p1)) ** k)
        lo = max(x for x in hs if x <= h); hi = min(x for x in hs if x >= h)
        if lo == hi:
            return power[lo]
        f = (h - lo) / (hi - lo)
        (a1, p1), (a2, p2) = power[lo], power[hi]
        return (a1 + (a2 - a1) * f, p1 + (p2 - p1) * f)

    new_rows = []
    for s in range(1, STAGES + 1):
        r = dict(cur[s])
        h = hour_of_stage(s)
        if h is None:
            # past the launch cap: continue the last decade's slope
            prev = new_rows[-1]; back = new_rows[-11]
            for k in ("hp", "atk", "xp"):
                r[k] = int(round(prev[k] * (prev[k] / max(1, back[k])) ** 0.1))
        else:
            pa, ph = party_at(h)
            r["hp"] = max(1, int(round(c_hp * pa / pop(s))))
            r["atk"] = max(1, int(round(c_atk * ph / pop(s))))
            r["xp"] = max(1, int(round(r["hp"] * xp_ratio)))
        new_rows.append(r)
    print(f"power samples {len(hours)} (h{hours[0]}..h{hours[-1]}: atk {power[hours[0]][0]}..{power[hours[-1]][0]}, hp {power[hours[0]][1]}..{power[hours[-1]][1]})")
    for s in (1, 12, 30, 55, 75, 92, 106, 118, 139, 168, 200, 240):
        o, n = cur[s], new_rows[s - 1]
        print(f"stage {s:>3}: hp {o['hp']:>7} -> {n['hp']:>7}   atk {o['atk']:>5} -> {n['atk']:>5}   xp {o['xp']:>5} -> {n['xp']:>5}   pop {pop(s)}")
    if write:
        data["stage_base"] = new_rows
        json.dump(data, open("data/game_data.json", "w"), indent=2, ensure_ascii=False)
        open("data/game_data.json", "a").write("\n")
        print("written")


POWER_MARGIN = 1.0


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(2)
    path = sys.argv[1]
    write = "--write" in sys.argv
    global POWER_MARGIN
    for a in sys.argv:
        if a.startswith("--margin="):
            POWER_MARGIN = float(a.split("=", 1)[1])
    if "--power" in sys.argv:
        rows = list(csv.DictReader(open(path)))
        fit_power(rows, json.load(open("data/game_data.json")), write)
        return
    rows = list(csv.DictReader(open(path)))
    reached = {}
    best = 0
    for r in rows:
        h = int(r["tick"]) / 36000.0
        m = re.match(r"zone_b(\d+)_", r["zone"])
        st = int(m.group(1)) if m else 0
        best = max(best, st)
        reached[round(h)] = best
    hours = sorted(h for h in reached if h >= FIT_MIN_HOURS)
    # A STALL is not a measurement of what the party can handle — it is the wall. Cut the fit at
    # the first hour the max stage has not moved for `STALL_HOURS`; stages past that extrapolate
    # (and the next iteration, run on the refit table, measures them for real).
    stall_at = None
    for h in hours:
        if h - STALL_HOURS in reached and reached[h] == reached[h - STALL_HOURS]:
            stall_at = h - STALL_HOURS
            break
    if stall_at is not None:
        hours = [h for h in hours if h <= stall_at]
        print(f"stall from hour {stall_at} at stage {reached[stall_at]} — fitting only up to there")
    data = json.load(open("data/game_data.json"))
    cur = {r["stage"]: r for r in data["stage_base"]}

    # (target stage → current row the party handled at that hour), one sample per hour.
    samples = []
    for h in hours:
        s_meas = reached[h]
        s_tgt = target_stage(h)
        if s_meas < 1:
            continue
        samples.append((s_tgt, cur[min(s_meas, STAGES)]))
    # A stall (the same measured stage for many hours) would pin many target stages to one row;
    # keep only the FIRST hour each measured stage was reached — the moment it was beatable.
    seen = set()
    firsts = []
    for s_tgt, row in samples:
        if row["stage"] in seen:
            continue
        seen.add(row["stage"])
        firsts.append((s_tgt, row))
    firsts.sort(key=lambda x: x[0])

    def interp(key, s):
        pts = [(t, math.log(max(1, r[key]))) for t, r in firsts]
        if s <= pts[0][0]:
            return math.exp(pts[0][1])
        for (t0, v0), (t1, v1) in zip(pts, pts[1:]):
            if t0 <= s <= t1:
                f = (s - t0) / (t1 - t0) if t1 > t0 else 0
                return math.exp(v0 + (v1 - v0) * f)
        # past the last sample: slope of the last fitted span, geometric
        (ta, va), (tb, vb) = pts[-2], pts[-1]
        slope = (vb - va) / max(1e-9, (tb - ta))
        return math.exp(vb + slope * (s - tb))

    new_rows = []
    for s in range(1, STAGES + 1):
        r = dict(cur[s])
        first_t = firsts[0][0]
        if s < first_t:
            # below the first fitted stage: geometric from stage 1's current row up to the first sample
            r0 = cur[1]
            f = (s - 1) / max(1, first_t - 1)
            for k in ("hp", "atk", "xp"):
                a, b = max(1, r0[k]), max(1, interp(k, first_t))
                r[k] = int(round(math.exp(math.log(a) + (math.log(b) - math.log(a)) * f)))
        else:
            for k in ("hp", "atk", "xp"):
                r[k] = int(round(interp(k, s)))
        new_rows.append(r)

    print(f"samples {len(firsts)} (hours {hours[0]}..{hours[-1]}, measured stage {reached[hours[0]]}..{reached[hours[-1]]})")
    for s in (1, 12, 30, 55, 75, 92, 106, 118, 139, 168, 200, 240):
        o, n = cur[s], new_rows[s - 1]
        print(f"stage {s:>3}: hp {o['hp']:>7} -> {n['hp']:>7}   atk {o['atk']:>5} -> {n['atk']:>5}   xp {o['xp']:>5} -> {n['xp']:>5}")
    if write:
        data["stage_base"] = new_rows
        json.dump(data, open("data/game_data.json", "w"), indent=2, ensure_ascii=False)
        open("data/game_data.json", "a").write("\n")
        print("written")


if __name__ == "__main__":
    main()
