#!/usr/bin/env python3
"""Every symbol in the Godot arena renderer must be ACCOUNTED FOR in the Unity port.

Not "translated" — accounted for. A hand port drops things silently, and the failure is never a
compile error: the Unity file simply has no equivalent of a const nobody remembered. This walks
the Godot render path, extracts every const / func / top-level var / signal, and requires each one
to appear in docs/unity-port-ledger.csv with a status:

    ported     — has a Unity counterpart (note = where)
    deferred   — deliberately not ported (note = WHY; this is a decision, not a hole)
    missing    — a real gap: it should exist on the Unity side and does not (note = what)
    n/a        — Godot-engine plumbing with nothing to port (SubViewport wiring, _ready, …)

`deferred` and `missing` differ on purpose. Deferred is a decision someone made and can defend;
missing is work nobody has done yet. Collapsing them loses the only distinction that matters when
deciding whether the port is finished.

    tools/port_ledger.py            # verify: exit 1 if any symbol is unaccounted for
    tools/port_ledger.py --update   # add new symbols as TODO, keep existing statuses

`--update` never overwrites a status you set, and it flags rows whose Godot symbol has since
disappeared, so the ledger cannot rot into a list of names that no longer exist.
"""
import csv, os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LEDGER = os.path.join(ROOT, "docs/unity-port-ledger.csv")
# The arena RENDER PATH only. The rest of godot/views (inventory, shop, HUD …) is a different
# port with a different schedule; listing it here would drown the signal for the part in flight.
# The HD-2D (3D) render path — what the Unity arena is a port OF. `views/enemy_animator.gd` and
# `views/base_monster.gd` are the 2D renderer's equivalents and are deliberately NOT listed: that
# renderer is frozen (CLAUDE.md), and `base_monster_3d.gd` is the copy that matters. `arena3d_fx.gd`
# is out too — VFX is its own port with its own schedule.
SOURCES = [
    "godot/proto/arena3d/arena3d_scene.gd",
    "godot/proto/arena3d/arena3d.gd",
    "godot/proto/arena3d/player_node_3d.gd",
    "godot/proto/arena3d/base_monster_3d.gd",
    "godot/proto/arena3d/player_animator_3d.gd",
    "godot/views/player_animator.gd",
]
UNITY = ["unity/ROI/Assets/Scripts", "unity/ROI/Assets/Editor"]

SYM = [
    (re.compile(r"^const\s+([A-Za-z_][A-Za-z0-9_]*)"), "const"),
    (re.compile(r"^var\s+([A-Za-z_][A-Za-z0-9_]*)"), "var"),
    (re.compile(r"^@onready\s+var\s+([A-Za-z_][A-Za-z0-9_]*)"), "var"),
    (re.compile(r"^(?:static\s+)?func\s+([A-Za-z_][A-Za-z0-9_]*)"), "func"),
    (re.compile(r"^signal\s+([A-Za-z_][A-Za-z0-9_]*)"), "signal"),
]


def godot_symbols():
    out = []
    for rel in SOURCES:
        path = os.path.join(ROOT, rel)
        for line in open(path):
            for rx, kind in SYM:
                m = rx.match(line)
                if m:
                    out.append((rel, m.group(1), kind))
                    break
    return out


def unity_text():
    buf = []
    for d in UNITY:
        for dirpath, _, files in os.walk(os.path.join(ROOT, d)):
            for f in files:
                if f.endswith(".cs"):
                    buf.append(open(os.path.join(dirpath, f)).read())
    return "\n".join(buf)


def squash(s):
    return s.replace("_", "").lower()


def load():
    if not os.path.exists(LEDGER):
        return {}
    with open(LEDGER) as f:
        return {(r["file"], r["symbol"]): r for r in csv.DictReader(f)}


def save(rows):
    with open(LEDGER, "w", newline="") as f:
        w = csv.DictWriter(f, ["file", "symbol", "kind", "status", "note"])
        w.writeheader()
        for r in rows:
            w.writerow(r)


def main(update):
    syms = godot_symbols()
    have = load()
    if update:
        # A name that survives squashing into the C# sources is a STRONG hint, never a verdict:
        # the seeded status is "ported?" so the question mark has to be resolved by a human.
        blob = squash(unity_text())
        rows, seeded = [], 0
        for rel, name, kind in syms:
            key = (rel, name)
            if key in have:
                rows.append(have.pop(key))
                continue
            guess = "ported?" if len(name) > 4 and squash(name) in blob else "TODO"
            seeded += guess == "ported?"
            rows.append({"file": rel, "symbol": name, "kind": kind, "status": guess, "note": ""})
        # A row for a file no longer in SOURCES is out of scope, not a missing symbol: drop it
        # rather than parading it as GONE forever.
        have = {k: v for k, v in have.items() if k[0] in SOURCES}
        for (rel, name), r in have.items():
            r["status"] = "GONE(" + r["status"] + ")"
            r["note"] = (r["note"] + " | symbol no longer in " + rel).strip(" |")
            rows.append(r)
        save(rows)
        print(f"ledger: {len(rows)} rows ({seeded} seeded as 'ported?', {len(have)} now GONE)")
        return 0

    missing = [(f, s, k) for f, s, k in syms if (f, s) not in have]
    for f, s, k in missing:
        print(f"UNACCOUNTED {k} {s}  ({f})")
    counts = {}
    for r in have.values():
        counts[r["status"]] = counts.get(r["status"], 0) + 1
    print("ledger status:", ", ".join(f"{k}={v}" for k, v in sorted(counts.items())))
    if missing:
        print(f"\n{len(missing)} Godot symbol(s) not in {os.path.relpath(LEDGER, ROOT)} — "
              "run tools/port_ledger.py --update and give each one a status.")
        return 1
    gone = [k for k, r in have.items() if r["status"].startswith("GONE")]
    if gone:
        print(f"{len(gone)} ledger row(s) reference a symbol that no longer exists.")
        return 1
    # Every row is triaged today, so an UNTRIAGED row is new work that arrived without a decision
    # — which is the whole failure this file exists to stop. `ported?` is the seeder's guess and
    # counts as untriaged until someone confirms it.
    untriaged = [k for k, r in have.items() if r["status"] in ("TODO", "ported?")]
    for f, sym in untriaged:
        print(f"UNTRIAGED {sym}  ({f})")
    if untriaged:
        print(f"\n{len(untriaged)} row(s) still need a status "
              "(ported / deferred / missing / n/a).")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main("--update" in sys.argv))
