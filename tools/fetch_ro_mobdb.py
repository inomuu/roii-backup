#!/usr/bin/env python3.12
"""Fetch + parse the rAthena monster DB into our reference files.

PRE-RENEWAL by default; `--renewal` for the other one. Both are wanted: the game's
monster HP/ATK/DEF are a pre-re port (0.0.111, deliberately — a five-member party
is not RO's one), while its FORMULAS are renewal. Comparing the two is how you tell
which half a number belongs to.

Output (docs/reference/):
  ro_prere_monsters.csv / .json      (default)
  ro_renewal_monsters.csv / .json    (--renewal)

Source: rAthena db/pre-re/mob_db.yml (GPL). We store only the DERIVED data
(stat numbers = facts) + this script, crediting the source — not the raw GPL file.
Run:  python3.12 tools/fetch_ro_mobdb.py
"""
import csv, json, os, sys, urllib.request
import yaml

RENEWAL = "--renewal" in sys.argv
URL = ("https://raw.githubusercontent.com/rathena/rathena/master/db/"
       + ("re" if RENEWAL else "pre-re") + "/mob_db.yml")
PREFIX = "ro_renewal" if RENEWAL else "ro_prere"
OUTDIR = "docs/reference"
RAW = "/tmp/mob_db_%s.yml" % ("re" if RENEWAL else "prere")

# Scalar columns to flatten into the CSV (in a sensible order).
COLS = [
    "Id", "AegisName", "Name", "Level", "Hp", "Sp",
    "BaseExp", "JobExp", "MvpExp",
    "Attack", "Attack2", "Defense", "MagicDefense", "Resistance", "MagicResistance",
    "Str", "Agi", "Vit", "Int", "Dex", "Luk",
    "Hit", "Flee",  # derived: Hit = Level+Dex, Flee = Level+Agi (not stored in DB)
    "AttackRange", "SkillRange", "ChaseRange",
    "Size", "Race", "Element", "ElementLevel",
    "WalkSpeed", "AttackDelay", "AttackMotion", "DamageMotion", "DamageTaken",
    "Ai", "Class", "IsMvp", "IsBoss", "DropCount",
]

def main():
    if not os.path.exists(RAW):
        print(f"downloading {URL} …")
        urllib.request.urlretrieve(URL, RAW)
    doc = yaml.safe_load(open(RAW))
    body = doc["Body"]
    os.makedirs(OUTDIR, exist_ok=True)

    rows = []
    for m in body:
        modes = m.get("Modes", {}) or {}
        row = {c: m.get(c, "") for c in COLS}
        row["IsMvp"] = bool(modes.get("Mvp", False))
        row["IsBoss"] = (m.get("Class", "") == "Boss") or bool(modes.get("Mvp", False))
        row["DropCount"] = len(m.get("Drops", []) or [])
        # Hit/Flee aren't stored — derive the engine raw values (pre-re mob):
        #   Hit = Level + Dex   ·   Flee = Level + Agi
        lvl = int(m.get("Level", 1) or 1)
        row["Hit"] = lvl + int(m.get("Dex", 0) or 0)
        row["Flee"] = lvl + int(m.get("Agi", 0) or 0)
        # defaults the YAML omits
        if row["Defense"] == "": row["Defense"] = 0
        if row["MagicDefense"] == "": row["MagicDefense"] = 0
        if row["ElementLevel"] == "": row["ElementLevel"] = 1
        rows.append(row)

    rows.sort(key=lambda r: (int(r["Level"] or 0), int(r["Id"])))

    csv_path = os.path.join(OUTDIR, PREFIX + "_monsters.csv")
    with open(csv_path, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=COLS)
        w.writeheader()
        w.writerows(rows)

    json_path = os.path.join(OUTDIR, PREFIX + "_monsters.json")
    json.dump(body, open(json_path, "w"), ensure_ascii=False, indent=1)

    print(f"{len(body)} monsters")
    print(f"  -> {csv_path}")
    print(f"  -> {json_path}")

if __name__ == "__main__":
    sys.exit(main())
