#!/usr/bin/env python3
"""Loc key guard (L10N key-only policy).

Enforces, across godot/**/*.gd:
  1. every LITERAL key passed to Loc.ui(...) exists in godot/i18n/strings.csv —
     with key-only calls a missing key renders as the raw KEY on screen;
  2. no new legacy 2-arg literal calls Loc.ui("KEY", "EN literal") — the CSV `en`
     column is the single source of the English (retrofitted 2026-07-22).
Dynamic keys (e.g. "ZONE_NAME_b%d" % band) can't be checked statically and keep
their runtime fallback — they are skipped.

AND, across unity/ROI/Assets/Scripts/**/*.cs, the same rule for the C# client:
every literal key given to Loc.Get / Loc.GetOr / Loc.Fmt must exist in the CSV.
The two clients read ONE table, so one guard covers both.
Unity story [ID:...] keys must exist too: keyed dialogue discards its inline draft,
so a missing translation previously produced completely blank choice buttons.

  Why the C# half was added late: the port shipped ~400 English literals straight
  into the UI while this file only ever looked at .gd, so nothing went red. The
  guard now sees the client that is being written.

Run from the repo root:  python3 tools/check_loc_keys.py   (exit 1 on violation)
"""
import csv
import glob
import re
import sys

CSV = "godot/i18n/strings.csv"
# The daily board lives in Rust; its ids become loc keys by concatenation on the client.
DAILY_RS = "crates/verify-server/src/daily.rs"

# The literal must be the WHOLE first argument (followed by `,` or `)`) — a trailing `+`
# means a concatenated dynamic key ("CLASS_BLURB_" + id), which keeps its runtime fallback.
LITERAL_KEY = re.compile(r'Loc\.ui\(\s*"([A-Za-z0-9_]+)"\s*[,)]')
LEGACY_2ARG = re.compile(r'Loc\.ui\(\s*"([A-Za-z0-9_]+)"\s*,\s*"((?:[^"\\]|\\.)*)"\s*\)', re.S)
# RAW keys set directly on Controls (`title.text = "UI_SHOP_TITLE"`) — the engine's
# auto-translate resolves them at draw time (and re-translates on a locale switch), so
# they must exist in the CSV too. Keys ending in `_` are concat prefixes — skipped.
RAW_KEY = re.compile(r'"((?:UI|STAT_DESC)_[A-Z0-9_]*[A-Z0-9])"')

# The C# client. Same rule, three call names — `GetOr` carries a fallback for the places where a
# missing entry HAS a sensible answer already (an item whose name the catalog carries), so a
# fallback there is not the legacy 2-arg smell it is in GDScript. What matters either way is that
# a literal key resolves.
CS_KEY = re.compile(r'Loc\.(?:Get|GetOr|Fmt)\(\s*"([A-Za-z0-9_]+)"\s*[,)]')

def strip_cs_comments(src: str) -> str:
    return "\n".join(l for l in src.split("\n") if not l.lstrip().startswith("//"))

def strip_comments(src: str) -> str:
    # Drop comment-only lines (doc examples like `Loc.ui("KEY")` in loc.gd/main.gd).
    return "\n".join(l for l in src.split("\n") if not l.lstrip().startswith("#"))

def main() -> int:
    with open(CSV, encoding="utf-8") as f:
        keys = {row[0] for row in csv.reader(f) if row and row[0] != "keys"}

    missing = []   # (file, key)
    legacy = []    # (file, key)
    for path in glob.glob("godot/**/*.gd", recursive=True):
        src = strip_comments(open(path, encoding="utf-8").read())
        for m in LITERAL_KEY.finditer(src):
            if m.group(1) not in keys:
                missing.append((path, m.group(1)))
        for m in RAW_KEY.finditer(src):
            if m.group(1) not in keys:
                missing.append((path, m.group(1)))
        for m in LEGACY_2ARG.finditer(src):
            legacy.append((path, m.group(1)))

    for path in glob.glob("unity/ROI/Assets/Scripts/**/*.cs", recursive=True):
        src = strip_cs_comments(open(path, encoding="utf-8").read())
        for m in CS_KEY.finditer(src):
            if m.group(1) not in keys:
                missing.append((path, m.group(1)))

    for path in glob.glob("unity/ROI/Assets/StreamingAssets/story/**/*.dialogue", recursive=True):
        src = strip_comments(open(path, encoding="utf-8").read())
        for key in re.findall(r"\[ID:\s*([^\]]+?)\s*\]", src):
            if key not in keys:
                missing.append((path, key))

    # DYNAMIC KEY, checked from the other side. `daily_view` builds its objective names as
    # "UI_DAILY_Q_" + the board id, which LITERAL_KEY cannot see — so the board is read from
    # daily.rs and each id checked for its key. Two files that must agree, asserted together:
    # a renamed commission would otherwise render as the raw key with nothing going red.
    board_ids = re.findall(r'd\("([a-z0-9_]+)"', open(DAILY_RS, encoding="utf-8").read())
    if not board_ids:
        print(f"✗ found no commission ids in {DAILY_RS} — this check is not looking where the board is")
        return 1
    for bid in board_ids:
        k = "UI_DAILY_Q_" + bid
        if k not in keys:
            missing.append((DAILY_RS, k))

    ok = True
    if missing:
        ok = False
        print(f"✗ {len(missing)} localization key(s) NOT in {CSV} (would render blank or as the raw key):")
        for path, key in sorted(set(missing)):
            print(f"    {key}  ({path})")
    if legacy:
        ok = False
        print(f"✗ {len(legacy)} legacy 2-arg Loc.ui(key, \"EN\") call(s) — add the key to "
              f"{CSV} and use key-only Loc.ui(key):")
        for path, key in sorted(set(legacy)):
            print(f"    {key}  ({path})")
    if ok:
        print(f"✓ loc keys ok ({len(keys)} CSV keys; every literal key in .gd, .cs and Unity dialogue is present, "
              f"no legacy 2-arg calls)")
    return 0 if ok else 1

if __name__ == "__main__":
    sys.exit(main())
