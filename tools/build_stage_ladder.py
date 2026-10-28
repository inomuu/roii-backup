#!/usr/bin/env python3
"""build_stage_ladder.py — generate `stage_base` from the party's REACHABLE power on each stage.

Two measured rounds (0.0.398-0.0.399) showed why a fixed slope walls: the party's ATK is almost
entirely GEAR, and gear TIER is gated by stage (`loot_rules.gear.bands`). A ladder that is steeper
than the tier the party can be wearing at that stage is a wall whatever the slope — round 1 walled
at stage 36 with tier-1 gear, the geometric table at ~100 with tier-3. So the table is built the
other way round: for every stage, what can a party wearing THIS stage's gear at a typical grade,
at the level the pace puts it on, actually hit for — and a monster is a fixed number of those hits.

    member_atk(s) = base(L(s)) + weapon(s) × grade(s) + regalia(s)
    mob_hp(s)     = K(s) × party_atk(s)           K: swings from the WHOLE party to kill (0.0.419), 6 → 12 → 14 → 16
    mob_atk(s)    = J(s) × member_hp(L(s))        J: 12% of a member's HP per ROOM-hit (÷ population) → 16% at the cap
    xp(s)         = hp(s) × r(s)                  r: 3.3% → 1% at the cap (later levels come slower)

`weapon(s)` ramps geometrically from the PREVIOUS tier's ATK at the start of a band to this tier's
at its end — the party enters a band still wearing the last one. `grade(s)` is the worn grade a
fusing player has: Uncommon by 10, Rare by 40, Legendary by 100, Immortal by the cap. `regalia(s)`
is the free R Regalia levelled along the way (100 → 1,500). `L(s)` is linear 1 → 60 to the cap.
def / toughness keep their stage index.

    tools/build_stage_ladder.py                    # report, model power
    tools/build_stage_ladder.py run.csv            # report, MEASURED power (sim-cli --sample 36000 run)
    tools/build_stage_ladder.py run.csv --write    # rewrite data/game_data.json (bump the version yourself)
"""
import json
import re, math, sys

STAGES, CAP = 240, 168
GRADE_MULT = {"Common": 1.0, "Uncommon": 1.3, "Rare": 1.69, "Legendary": 2.197, "Immortal": 2.856}
GRADE_ANCHORS = [(1, "Common"), (10, "Uncommon"), (40, "Rare"), (100, "Legendary"), (CAP, "Immortal"), (STAGES, "Immortal")]
# 0.0.408: ×2 (user: "HP ×2") — a room of 30 is a fight, not a sweep; XP, Gil and drop odds
# ride K so the hour pays the same.
# 0.0.416: ×3 again (user: "ลองเลือด ×3") — the REAL party (Seraphine + pulled Exemplars, AoE
# kits) ran the ×2 ladder to its end on day 3.
# 0.0.418 (user: "progress วัน 1 ไวไป" — stage 72 on day 1): the early ramp steepens, 6 → 45 by
# stage 30, and the top end lifts a quarter with it.
# K is HITS FROM THE WHOLE PARTY since 0.0.419 (`measured_power` sums the party's ATK), so the
# 1 → 5 members of the first hours are in the measurement and the anchors only say how many
# party-swings a monster takes: 6 solo at stage 1, 12 by stage 30, 14 at the cap, 16 at 240.
# 0.0.432 (user: "ช่วงต้นเกมง่ายไป แทบไม่มีติดหรือ wipe เลย — ช่วงด่าน 11-80"): a measured 7-day run
# wiped 22 times on stages 1-10 and only 21 times across 11-80, then 60 / 219 on 81-100. K now
# reaches 14 by stage 80 instead of the cap, and J climbs to 22% by 50 and 24% by 80 so the
# middle bites. At 81 J steps back to the old ramp (19% → 24% at the cap): 81+ already walls on
# the gothic arc's kits, and the arc boundary hides the step.
K_ANCHORS = [(1, 6.0), (30, 12.0), (80, 14.0), (CAP, 14.0), (STAGES, 16.0)]
# KILLS PER ROOM (the boss threshold) grow with the stage too — TBH's 10 → 30 waves: a stage
# takes longer late because there is more of it, not only because every monster is a sponge.
THRESHOLD_MULT = [(1, 1.0), (CAP, 1.0), (STAGES, 1.0)]
# J is the ROOM's incoming damage per hit as a share of a member's HP, split across the room's
# population: five monsters focusing one member at 6% each was 30% a second — the wall at
# stage ~110 in every run. 12% ÷ pop keeps the focused member alive ~8 s whatever the room size.
# 0.0.407 (user: "WoG มอนจะไม่ attack แรงมาก แต่จะค่อยๆ ตีให้เลือดลดลง" / "เดเมจมอนสเกลไว ...
# อยู่ดีๆ ความยากพุ่ง"): FLAT 6% — half the old floor and no ramp, so a room chips the same share
# of a member's HP on every stage and difficulty rises only through K (hits to kill) and the
# room's kill count.
# 0.0.409: flat 6% had no wall at all (cap on day 4) — a soft ramp, half the old one's slope.
# 0.0.414: region 1 starts at 3% — a lone Lv3 lead was dying in stage 2's third room.
# 0.0.416: ×2 (user: "เพิ่มเดเมจมอนสเตอร์") — the real kits carry Aegis / Bulwark / heals the
# class harness never had, and the party barely died.
J_ANCHORS = [(1, 0.06), (12, 0.15), (50, 0.22), (80, 0.24), (81, 0.19), (CAP, 0.24), (STAGES, 0.30)]
# 0.0.409: ×0.5 (user: "lv ไวไป" — Lv61 on day 7 against TBH's Lv70 in two weeks); level is
# the HP the party out-sustains the ramp with.
# 0.0.416: ÷2 again (user: "เลเวลก็ไวเกิน xp / 2 อีกรอบ") — and XP rides HP, so with K ×3 the
# per-kill number is ×1.5 of 0.0.415's while the hour pays ~¾ of it.
# 0.0.417: ÷2 once more (user: "ยังไวไป ลองลด exp อีก 50%") — Lv54 on day 7 against Lv70 at two weeks.
XP_RATIO_ANCHORS = [(1, 0.004125), (CAP, 0.00125), (STAGES, 0.001)]
LEVEL_AT_CAP, LEVEL_AT_END = 60, 80
REGALIA_LUMP = [(1, 100), (CAP, 1500), (STAGES, 3000)]


def geo(anchors, s):
    for (s0, v0), (s1, v1) in zip(anchors, anchors[1:]):
        if s0 <= s <= s1:
            f = (s - s0) / max(1, s1 - s0)
            return math.exp(math.log(v0) + (math.log(v1) - math.log(v0)) * f)
    return anchors[-1][1]


def lin(anchors, s):
    for (s0, v0), (s1, v1) in zip(anchors, anchors[1:]):
        if s0 <= s <= s1:
            f = (s - s0) / max(1, s1 - s0)
            return v0 + (v1 - v0) * f
    return anchors[-1][1]


TARGET_BY_DAY = [0, 30, 55, 75, 92, 106, 118, 129, 139, 148, 156, 162, 168]


def target_hour(stage):
    """The hour the pace puts the party on `stage` (linear inside each day); None past the cap."""
    for d in range(len(TARGET_BY_DAY) - 1):
        a, b = TARGET_BY_DAY[d], TARGET_BY_DAY[d + 1]
        if a <= stage <= b:
            return 24 * (d + (stage - a) / max(1, b - a))
    return None


def measured_power(path):
    """Per-member effective ATK and max HP BY STAGE — what the party had the first hour it stood
    on each stage — from a `sim-cli --sample 36000` run. Keyed by stage, not by hour: the party's
    power is mostly the gear its stage lets it wear, so this holds whatever pace it ran at."""
    import csv, re
    out = {}
    best = 0
    for r in csv.DictReader(open(path)):
        n = max(1, int(r["party"]))
        m = re.match(r"zone_b(\d+)_", r["zone"])
        st = int(m.group(1)) if m else 0
        if st > best:
            for s in range(best + 1, st + 1):
                # (the PARTY's summed ATK, one member's HP): the sum is what a room faces —
                # 0.0.419 (user: "ช่วงแรกสเกลไวได้เพราะได้ตัวละครเพิ่ม") — so the ladder climbs where
                # members join, not where a hand-placed anchor guessed they would.
                out[s] = (int(r["party_atk"]), int(r["party_hp"]) / n)
            best = st
    return out


# SMOOTH OPENING (user 2026-09-30: "ไม่ต้อง jump มากถึงแม้ว่า party จะเพิ่ม"): monster HP is sized
# from the party's summed ATK, so the measuring run's party growing from one member to five put a
# ×6 HP cliff between stages 1 and 2 and ×1.9 at 9→10. A new player's party grows over a day now
# (slots at stage 6/18/24), and the opening should not punish each arrival. Stages 1..OPEN_TO-1
# follow a geometric curve from OPEN_HP1 (HP) / the stage-1 ATK to stage OPEN_TO's own values; XP
# keeps each stage's XP-per-HP ratio. Fitted on sim-cli --gacha-flow with Suzu ALONE through stage 1
# (slot 2 opens at stage 2): 2,500 clears stage 1 by ~6 min without tutorial quest 1's weapon, 15 min
# ≈ stage 4, 2-3 wipes in the first hour (1-2 with the weapon). 3,200 walled the solo stage-1 boss.
OPEN_TO = 24
OPEN_HP1 = 2500


def smooth_opening(rows):
    by = {r["stage"]: r for r in rows}
    top = by[OPEN_TO]
    atk1 = by[1]["atk"]
    for s in range(1, OPEN_TO):
        r = by[s]
        f = (s - 1) / (OPEN_TO - 1)
        hp = int(round(OPEN_HP1 * (top["hp"] / OPEN_HP1) ** f))
        r["xp"] = max(1, int(round(r["xp"] * hp / r["hp"])))
        r["hp"] = hp
        r["atk"] = int(round(atk1 * (top["atk"] / atk1) ** f))
    return rows


def main():
    if "--smooth-opening" in sys.argv:
        # Apply the opening curve to the catalog as it stands, rebuilding nothing else.
        # Patched in place, number by number: a json.dump round-trip reflows the hand-compacted
        # arrays elsewhere in the catalog.
        text = open("data/game_data.json").read()
        d = json.loads(text)
        rows = smooth_opening([dict(r) for r in d["stage_base"]])
        start = text.index('"stage_base"')
        head, body = text[:start], text[start:]
        for r in rows[:OPEN_TO]:
            pat = re.compile(r'("stage": %d,\s*"hp": )\d+(,\s*"atk": )\d+(,\s*"xp": )\d+' % r["stage"])
            body, n = pat.subn(lambda m: f'{m.group(1)}{r["hp"]}{m.group(2)}{r["atk"]}{m.group(3)}{r["xp"]}', body, count=1)
            assert n == 1, f"stage {r['stage']} row not found"
        open("data/game_data.json", "w").write(head + body)
        print("opening smoothed:", [(r["stage"], r["hp"], r["atk"]) for r in rows[:OPEN_TO]])
        return
    write = "--write" in sys.argv
    power = None
    for a in sys.argv[1:]:
        if a.endswith(".csv"):
            power = measured_power(a)
    d = json.load(open("data/game_data.json"))
    items = {i["id"]: i for i in d["items"]}
    sword = [items[f"sword_s{t}"]["atk_bonus"] for t in range(1, 8)]
    bands = d["loot_rules"]["gear"]["bands"]
    unit = next(u for u in d["units"] if u["id"] == "seraphine")

    def tier_of(s):
        for b in bands:
            if s <= b["max_stage"]:
                return b["tier"]
        return bands[-1]["tier"]

    def band_span(s):
        lo = 1
        for b in bands:
            if s <= b["max_stage"]:
                return lo, min(b["max_stage"], STAGES)
            lo = b["max_stage"] + 1
        return lo, STAGES

    zones = {z["stage"]: z for z in d["zones"] if z["id"].startswith("zone_b") and z["id"].endswith("_1")}

    def pop(s):
        return max(1, zones.get(s, zones[1]).get("target_population", 2))

    def level(s):
        return lin([(1, 1), (CAP, LEVEL_AT_CAP), (STAGES, LEVEL_AT_END)], s)

    def base_atk(L):
        return (unit["atk_base_milli"] + unit["atk_per_level_milli"] * (L - 1)) / 1000.0

    def member_hp(L):
        hp = unit["hp"]
        for l in range(2, int(L) + 1):
            hp += (unit["hp_increase"] + unit["hp_factor"] * l) / 100.0
        return hp

    def weapon(s):
        t = tier_of(s)
        lo, hi = band_span(s)
        prev = sword[max(0, t - 2)] if t > 1 else sword[0] * 0.6
        cur = sword[t - 1]
        f = (s - lo) / max(1, hi - lo)
        return math.exp(math.log(prev) + (math.log(cur) - math.log(prev)) * f)

    def grade(s):
        pts = [(st, GRADE_MULT[g]) for st, g in GRADE_ANCHORS]
        return geo(pts, s)

    def model_atk(s):
        return base_atk(level(s)) + weapon(s) * grade(s) + geo(REGALIA_LUMP, s)

    def power_at(s):
        """Measured member (atk, hp) on stage s; past the run's reach, the gear MODEL's growth
        scaled to match at the last measured stage."""
        top = max(power)
        if s <= top:
            return power[s]
        a_top, h_top = power[top]
        # Past the run's reach the SUM grows like one member's model ATK (the party is full).
        return (a_top * model_atk(s) / model_atk(top), h_top * member_hp(level(s)) / member_hp(level(top)))

    rows = []
    old = {r["stage"]: r for r in d["stage_base"]}
    for s in range(1, STAGES + 1):
        L = level(s)
        if power is not None:
            # MEASURED: the member ATK / HP the party actually has at the hour the pace puts it
            # on this stage (a sim-cli run's `party_atk` / `party_hp`), instead of the model's.
            atk_m, _ = power_at(s)
            # Member HP from the MODEL even in measured mode (0.0.407): a run that stands on one
            # stage for days keeps levelling, so measured HP by stage steps up wherever the run
            # stalled — and the room's ATK, sized to it, spiked ×2 across stages 100-120.
            hp_m = member_hp(L)
        else:
            # Model mode: a full party of five from stage 1 (no run to read the roster from).
            atk_m = 5 * (base_atk(L) + weapon(s) * grade(s) + geo(REGALIA_LUMP, s))
            hp_m = member_hp(L)
        hp = int(round(lin(K_ANCHORS, s) * atk_m))
        atk = int(round(lin(J_ANCHORS, s) * hp_m / pop(s)))
        xp = max(1, int(round(hp * lin(XP_RATIO_ANCHORS, s))))
        r = dict(old[s])
        r.update({"hp": hp, "atk": atk, "xp": xp})
        rows.append(r)
    for s in (1, 10, 24, 44, 45, 60, 61, 90, 120, 121, 150, 168, 200, 240):
        o, n = old[s], rows[s - 1]
        print(f"stage {s:>3} (tier {tier_of(s)}, Lv{level(s):>4.0f}, member atk {(power_at(s)[0] if power else base_atk(level(s)) + weapon(s) * grade(s) + geo(REGALIA_LUMP, s)):>6.0f}): "
              f"hp {o['hp']:>7} -> {n['hp']:>7}   atk {o['atk']:>5} -> {n['atk']:>5}   xp {o['xp']:>5} -> {n['xp']:>5}")
    # DROP ODDS PER KILL ∝ K (user 2026-09-17: "ฟาร์มด่าน under lv ต้องคุ้มน้อยกว่า on level"): a
    # one-hit kill pays a quarter of a four-hit kill, so drops per HOUR are flat across stages and
    # the on-level stage wins on tier, grade tail and XP alone — no penalty needed. Gear slot odds
    # and gem odds both, in rows of eight stages.
    gear = d["loot_rules"]["gear"]
    base_field, base_boss = 12, 24
    slot_rows, gem_rows = [], []
    gbase = d["loot_rules"]["gems"]
    for top in list(range(8, STAGES, 8)) + [STAGES]:
        k = lin(K_ANCHORS, top) / lin(K_ANCHORS, 1)
        slot_rows.append({"max_stage": top, "field_slot_bp": int(round(base_field * k)), "boss_slot_bp": int(round(base_boss * k))})
        gem_rows.append({"max_stage": top, "field_bp": int(round(20 * k)), "boss_bp": int(round(40 * k))})
    print("slot_bp rows:", [(r["max_stage"], r["field_slot_bp"]) for r in slot_rows[::5]])
    # boss_threshold by stage: the region's authored count × THRESHOLD_MULT (rooms 1-3 keep
    # their +0/+1/+2 shape). Authored counts live on the first stage of each region.
    base_thr = {}
    for z in d["zones"]:
        if z["id"].startswith("zone_b") and z["stage"] % 12 == 1:
            base_thr[(z["stage"] - 1) // 12 + 1] = base_thr.get((z["stage"] - 1) // 12 + 1) or z["boss_threshold"]
    # 0.0.414 (user: "ด่าน 1 ควรจะสัก 20 คิล แล้วสเกลขึ้นเลย"): 20 kills on stage 1 rising one per
    # stage-and-a-bit to 30 by stage 12, flat 30 after; rooms 2/3 +1/+2. Authored here, not read
    # from the regions' first stages any more.
    for z in d["zones"]:
        if not z["id"].startswith("zone_b"):
            continue
        st = z["stage"]; room = int(z["id"].rsplit("_", 1)[1])
        z["boss_threshold"] = 20 + min(10, (st - 1) * 10 // 11) + (room - 1)
    print("boss_threshold:", [(s, next(z["boss_threshold"] for z in d["zones"] if z["id"] == f"zone_b{s}_1")) for s in (1, 30, 90, 168, 240)])
    smooth_opening(rows)
    if write:
        d["stage_base"] = rows
        gear["slot_bp_by_stage"] = slot_rows
        gbase["field_bp_by_stage"] = gem_rows
        json.dump(d, open("data/game_data.json", "w"), indent=2, ensure_ascii=False)
        open("data/game_data.json", "a").write("\n")
        print("written")


if __name__ == "__main__":
    main()
