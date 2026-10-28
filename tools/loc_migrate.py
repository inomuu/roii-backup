#!/usr/bin/env python3
"""Lift the C# client's on-screen English into loc keys.

The Unity port shipped its chrome as string literals — the Godot client's key-only rule was
enforced by `check_loc_keys.py`, which only ever read `.gd`. This does the mechanical half of the
migration: mint a key per literal, add a CSV row carrying the EXISTING English, and rewrite the
call site to `Loc.Get(KEY)`.

It writes the `en` column only. The `th` column is left EMPTY on purpose: `Loc` already falls back
to English for an untranslated row, so the game reads exactly as it does today, and the Thai is a
translation pass by someone who speaks it rather than a guess made by this script.

  python3 tools/loc_migrate.py --plan     # what it would do
  python3 tools/loc_migrate.py --apply    # do it
"""
import csv, glob, os, re, sys

CSV_PATH = "godot/i18n/strings.csv"
ROOT = "unity/ROI/Assets/Scripts"

# Four ways a plain literal reaches the screen. An interpolated $"..." is deliberately out of
# scope: it carries values and needs a format key, which is a different (and smaller) job.
PATS = [
    re.compile(r'(UiTheme\.(?:Label|Caps)\s*\(\s*[^,]+,\s*"[^"]*"\s*,\s*)"((?:[^"\\]|\\.)*)"'),
    re.compile(r'((?:HudBar\.)?Make(?:Ghost)?Button\s*\(\s*[^,]+,\s*)"((?:[^"\\]|\\.)*)"'),
    re.compile(r'(\.text\s*=\s*)"((?:[^"\\]|\\.)*)"(\s*;)'),
]

# WORDS THAT ARE THE SAME EVERYWHERE get one key, not one per screen. A translator should see
# "Cancel" once, and a screen that says Cancel differently from the screen next to it is a bug.
SHARED = {
    "Cancel": "UI_COMMON_CANCEL", "Close": "UI_COMMON_CLOSE", "Buy": "UI_COMMON_BUY",
    "Sell": "UI_COMMON_SELL", "Bag": "UI_COMMON_BAG", "Gear": "UI_COMMON_GEAR",
    "Equip": "UI_COMMON_EQUIP", "Unequip": "UI_COMMON_UNEQUIP", "Claim": "UI_COMMON_CLAIM",
    "Claimed": "UI_COMMON_CLAIMED", "Withdraw": "UI_COMMON_WITHDRAW",
    "Deposit": "UI_COMMON_DEPOSIT", "Discard": "UI_COMMON_DISCARD", "Apply": "UI_COMMON_APPLY",
    "List": "UI_COMMON_LIST", "Quit": "UI_COMMON_QUIT", "Menu": "UI_COMMON_MENU",
    "Commodities": "UI_COMMON_COMMODITIES", "Collection": "UI_COMMON_COLLECTION",
    "Connect to the local server": "UI_COMMON_CONNECT_LOCAL",
}

# NOT TEXT TO TRANSLATE. The game's name is a brand; an ALL-CAPS diagnostic is a developer state
# that no shipping build should reach; a bare fragment ("Lv ") is half of a sentence and belongs
# in a format key with the number it precedes.
def skip(text: str) -> bool:
    if text.strip() in ("Reign of Idlers", "ROI"): return True
    if re.fullmatch(r'[^a-z]*', text): return True          # ALL CAPS / punctuation only
    if len(text.strip()) < 2: return True
    if not re.search(r'[A-Za-z]{2}', text): return True
    if text.strip().endswith(" ") or text.strip() != text: return True
    return False

def screen_of(path: str) -> str:
    n = re.sub(r'Window$|Screen$', '', os.path.basename(path)[:-3])
    return re.sub(r'(?<!^)(?=[A-Z])', '_', n).upper()

def key_for(path: str, text: str) -> str:
    if text in SHARED: return SHARED[text]
    words = [w for w in re.sub(r'[^A-Za-z0-9 ]', ' ', text).split()][:5]
    return f"UI_{screen_of(path)}_" + "_".join(w.upper() for w in words)

def main() -> int:
    apply = "--apply" in sys.argv
    rows = list(csv.reader(open(CSV_PATH, encoding="utf-8")))
    have = {r[0] for r in rows if r}
    width = len(rows[0])
    new_rows, edits = {}, 0

    for path in sorted(glob.glob(f"{ROOT}/**/*.cs", recursive=True)):
        src = out = open(path, encoding="utf-8").read()
        for pat in PATS:
            def sub(m):
                nonlocal edits
                text = m.group(2)
                if skip(text): return m.group(0)
                k = key_for(path, text)
                if k not in have: new_rows[k] = text
                edits += 1
                tail = m.group(3) if pat.groups >= 3 and m.lastindex >= 3 else ""
                return f'{m.group(1)}Loc.Get("{k}"){tail}'
            out = pat.sub(sub, out)
        if out != src and apply:
            open(path, "w", encoding="utf-8").write(out)

    print(f"{edits} call sites, {len(new_rows)} new keys")
    if apply:
        with open(CSV_PATH, "a", encoding="utf-8", newline="") as f:
            w = csv.writer(f, lineterminator="\n")
            for k, text in sorted(new_rows.items()):
                row = [k, ""] + [text] + [""] * (width - 3)
                w.writerow(row)
        print(f"appended {len(new_rows)} rows to {CSV_PATH} (en filled, th empty)")
    else:
        for k, t in sorted(new_rows.items())[:12]:
            print(f"   {k:48} {t[:56]!r}")
    return 0

if __name__ == "__main__":
    sys.exit(main())
