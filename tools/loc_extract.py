#!/usr/bin/env python3
"""loc_extract.py — sync translation keys into godot/i18n/strings.csv.

Sources (docs/localization-plan.md):
  - data/game_data.json         -> ITEM_<id> / MON_<id> / ZONE_<id> (field `name`)
  - sim-cli --dump-strings      -> SKILL_<id> / SKILL_<id>_B (Rust catalog)
  - UI_* keys are hand-managed in the CSV; this script never touches them.

Behaviour:
  - New source keys are appended with their EN text; existing translations are
    NEVER modified or deleted.
  - If a source's EN text changed, the `en` column is updated and the row is
    reported (its translations are now stale).
  - Keys that vanished from the source are reported as orphans (not deleted —
    delete by hand once you're sure).
  - Prints per-locale untranslated counts at the end.

Usage: python3 tools/loc_extract.py           (from the repo root)
       python3 tools/loc_extract.py --check   (report only; exit 1 if CSV stale)
"""

import csv
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CSV_PATH = ROOT / "godot" / "i18n" / "strings.csv"
GAME_DATA = ROOT / "data" / "game_data.json"

# Column order = Godot locale codes. EN first (source of truth).
LOCALES = ["en", "th", "zh_CN", "zh_TW", "pt_BR", "ru", "ja", "ko",
           "es", "de", "fr", "pl", "tr", "vi", "id", "it", "uk"]

CATALOG_PREFIXES = ("ITEM_", "MON_", "ZONE_", "SKILL_")


def default_flags(key: str) -> str:
    """Translation policy by namespace (docs/localization-plan.md — provisional):
    item/monster/skill NAMES stay EN in every locale (MMO trade/guide vocabulary
    is cross-language; Steam Market wants one name worldwide). Skill BLURBS,
    zones, and UI text are the translation surface. Override per key by editing
    the `flags` cell — this default only applies when a key first appears.
    """
    if key.startswith("SKILL_") and key.endswith(("_B", "_D")):
        return ""      # blurb + lore detail = translation surface
    if key.startswith(("ITEM_", "MON_", "SKILL_")):
        return "fixed"
    return ""


def source_keys() -> dict:
    """key -> EN text, from game_data.json + sim-cli --dump-strings."""
    out = {}
    data = json.loads(GAME_DATA.read_text())
    for section, prefix in (("items", "ITEM_"), ("monsters", "MON_"), ("zones", "ZONE_")):
        for entry in data.get(section, []):
            eid, name = entry.get("id", ""), entry.get("name", "")
            if eid and name:
                out[prefix + eid] = name
    dump = subprocess.run(
        ["cargo", "run", "-q", "-p", "sim-cli", "--", "--dump-strings"],
        cwd=ROOT, capture_output=True, text=True, check=True)
    for row in csv.reader(dump.stdout.splitlines()):
        if len(row) == 2 and row[0]:
            out[row[0]] = row[1]
    return out


def load_csv() -> dict:
    """key -> {locale: text, "flags": str}. Empty dict if the CSV doesn't exist
    yet. Tolerates the pre-flags format (column added 2026-07-08)."""
    if not CSV_PATH.exists():
        return {}
    rows = {}
    with CSV_PATH.open(newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        key_col = reader.fieldnames[0]           # header cell name is arbitrary
        for row in reader:
            key = row.pop(key_col)
            if key:
                rows[key] = {k: (v or "") for k, v in row.items()}
                rows[key].setdefault("flags", default_flags(key))
    return rows


def main() -> int:
    check_only = "--check" in sys.argv
    src = source_keys()
    rows = load_csv()

    added, changed = [], []
    for key, en in sorted(src.items()):
        if key not in rows:
            rows[key] = {loc: "" for loc in LOCALES}
            rows[key]["en"] = en
            rows[key]["flags"] = default_flags(key)
            added.append(key)
        elif rows[key].get("en", "") != en:
            rows[key]["en"] = en
            changed.append(key)

    orphans = [k for k in rows
               if k.startswith(CATALOG_PREFIXES) and k not in src]

    if not check_only:
        CSV_PATH.parent.mkdir(parents=True, exist_ok=True)
        with CSV_PATH.open("w", newline="", encoding="utf-8") as f:
            writer = csv.writer(f)
            writer.writerow(["keys", "flags"] + LOCALES)
            # UI_ keys first (hand-managed), then catalog keys — both sorted.
            ui = sorted(k for k in rows if not k.startswith(CATALOG_PREFIXES))
            cat = sorted(k for k in rows if k.startswith(CATALOG_PREFIXES))
            for key in ui + cat:
                writer.writerow([key, rows[key].get("flags", "")]
                                + [rows[key].get(loc, "") for loc in LOCALES])

    print(f"keys: {len(rows)} total | +{len(added)} added | "
          f"{len(changed)} EN-changed (translations stale) | {len(orphans)} orphaned")
    for k in changed:
        print(f"  EN CHANGED: {k}")
    for k in orphans:
        print(f"  ORPHAN (delete by hand if intended): {k}")
    # `fixed` rows are policy-EN (never translated); a `=` cell means
    # "intentionally EN in this locale" — neither counts as ค้างแปล.
    todo = [r for r in rows.values() if "fixed" not in r.get("flags", "")]
    untranslated = {loc: sum(1 for r in todo if r.get(loc, "") == "")
                    for loc in LOCALES[1:]}
    print(f"translation surface: {len(todo)} keys ({len(rows) - len(todo)} fixed)")
    print("untranslated:", " ".join(f"{loc}={n}" for loc, n in untranslated.items()))

    if check_only and (added or changed):
        print("CSV is stale — run: python3 tools/loc_extract.py")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
