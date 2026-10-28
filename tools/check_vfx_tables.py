#!/usr/bin/env python3
"""Static guards for the 3D VFX tables. Exit 1 on violation.

Everything these tables hold is a `const` dict literal in GDScript, so all of it is reachable
without launching Godot. That matters because the failure mode for every one of these is SILENT:
a missing scene path makes `_claim_pack` return null and the bolt simply never appears, a typo in
a paint id makes `paint_of` return `{}` and the skill flies unpainted, and a colour over 1.0
blooms into a white hole around the caster. None of them raise, none of them log.

`docs/vfx-notes.md` asks for the colour check in writing and it was never built; this is that,
plus the neighbouring checks that cost nothing once the parser exists.

Runs in well under a second. Wired into tools/smoke.sh beside check_loc_keys.py.
"""
import ast
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PACK = ROOT / "godot/proto/arena3d/fx/pack_vfx.gd"
GODOT = ROOT / "godot"
CATALOG = ROOT / "crates/sim-core/src/skills/catalog"
ELEMENTS = ROOT / "crates/sim-core/src/grid/mod.rs"

# `arena3d_scene.GLOW_THRESHOLD`. Anything past it blooms, and a repaint that overshoots turns an
# additive bolt a metre from the caster into a white hole with the mage inside.
GLOW_THRESHOLD = 1.0

problems: list[str] = []


def fail(msg: str) -> None:
    problems.append(msg)


def const_block(src: str, name: str) -> str:
    """The text of `const NAME := [ ... ]` / `{ ... }`, by bracket matching."""
    m = re.search(rf"^const {re.escape(name)} :?=\s*([\[{{])", src, re.M)
    if not m:
        fail(f"{name}: not found in {PACK.name}")
        return ""
    open_c = m.group(1)
    close_c = "]" if open_c == "[" else "}"
    i = m.end() - 1
    depth = 0
    for j in range(i, len(src)):
        if src[j] == open_c:
            depth += 1
        elif src[j] == close_c:
            depth -= 1
            if depth == 0:
                return src[i : j + 1]
    fail(f"{name}: unbalanced brackets")
    return ""


def str_map(block: str) -> dict[str, str]:
    """`"key": "value"` pairs at any depth — good enough for the flat routing tables."""
    return dict(re.findall(r'"([^"]*)"\s*:\s*"([^"]*)"', block))


def int_map(block: str) -> dict[str, int]:
    return {k: int(v) for k, v in re.findall(r'"([^"]*)"\s*:\s*(-?\d+)', block)}


def top_level_keys(block: str) -> list[str]:
    """Keys of a dict-of-dicts, by depth — nested keys must not be mistaken for entries."""
    keys, depth, i = [], 0, 0
    while i < len(block):
        c = block[i]
        if c in "{[":
            depth += 1
            if depth == 1:
                i += 1
                continue
        elif c in "}]":
            depth -= 1
        elif depth == 1 and c == '"':
            j = block.index('"', i + 1)
            after = block[j + 1 :].lstrip()
            if after.startswith(":"):
                keys.append(block[i + 1 : j])
            i = j
        i += 1
    return keys


def paint_entries(block: str) -> dict[str, str]:
    """Top-level PAINTS entries, id -> its body text."""
    out, depth, i, cur, start = {}, 0, 0, None, 0
    while i < len(block):
        c = block[i]
        if c in "{[":
            depth += 1
            if depth == 2 and cur:
                start = i
        elif c in "}]":
            depth -= 1
            if depth == 1 and cur:
                out[cur] = block[start : i + 1]
                cur = None
        elif depth == 1 and c == '"':
            j = block.index('"', i + 1)
            if block[j + 1 :].lstrip().startswith(":"):
                cur = block[i + 1 : j]
            i = j
        i += 1
    return out


def trail_len(body: str) -> int:
    m = re.search(r'"trail"\s*:\s*\[(.*?)\]', body, re.S)
    return m.group(1).count("Color(") if m else 0


def tint_len(body: str) -> int:
    m = re.search(r'"tint"\s*:\s*\[(.*?)\]', body, re.S)
    return m.group(1).count("Color(") if m else 0


def main() -> int:
    src = PACK.read_text()

    # --- A. every scene path resolves on disk -------------------------------------------------
    # The one that turns a licence removal or a pack rename from "no magic bolts, no error" into
    # a build failure.
    paths = sorted(set(re.findall(r'"(res://VFX/[^"]+)"', src)))
    for p in paths:
        if not (GODOT / p[len("res://") :]).exists():
            fail(f"missing scene: {p}")

    # --- B. paint ids resolve ------------------------------------------------------------------
    paints = top_level_keys(const_block(src, "PAINTS"))
    for table in ("BY_SKILL", "BY_ELEMENT_PAINT"):
        for key, pid in str_map(const_block(src, table)).items():
            if pid not in paints:
                fail(f'{table}["{key}"] = "{pid}" — no such entry in PAINTS')

    # --- C. skill ids exist in the sim ---------------------------------------------------------
    # A renamed skill otherwise loses its VFX quietly and flies the fallback fireball.
    sim_skills = set()
    for rs in CATALOG.glob("*.rs"):
        sim_skills.update(re.findall(r'^\s*id:\s*"([^"]+)"', rs.read_text(), re.M))
    if not sim_skills:
        fail("no skill ids found in the sim catalog — the guard would pass vacuously")
    for key in str_map(const_block(src, "BY_SKILL")):
        if key and key not in sim_skills:
            fail(f'BY_SKILL["{key}"] — no such skill in crates/sim-core')
    for name in re.findall(r'"([a-z_]+)"', const_block(src, "FLIGHT_OWNED")):
        if name not in sim_skills:
            fail(f'FLIGHT_OWNED "{name}" — no such skill in crates/sim-core')

    # --- D. elements exist, triplets are in range ----------------------------------------------
    elem_src = ELEMENTS.read_text()
    enum = re.search(r"pub enum Element \{(.*?)\n\}", elem_src, re.S)
    variants = set(re.findall(r"^\s*([A-Z][A-Za-z]*),", enum.group(1), re.M)) if enum else set()
    if not variants:
        fail("could not read Element from crates/sim-core — the guard would pass vacuously")
    n_triplets = const_block(src, "TRIPLETS").count('"proj"')
    for key, idx in int_map(const_block(src, "BY_ELEMENT")).items():
        if key not in variants:
            fail(f'BY_ELEMENT["{key}"] — not an Element variant')
        if not 0 <= idx < n_triplets:
            fail(f'BY_ELEMENT["{key}"] = {idx} — outside TRIPLETS (0..{n_triplets - 1})')
    for elem in str_map(const_block(src, "BY_ELEMENT_PAINT")):
        if elem not in variants:
            fail(f'BY_ELEMENT_PAINT["{elem}"] — not an Element variant')
    for m in re.finditer(r'"triplet"\s*:\s*(\d+)', src):
        if int(m.group(1)) >= n_triplets:
            fail(f'a paint\'s "triplet": {m.group(1)} is outside TRIPLETS')

    # --- E. no painted colour blooms -----------------------------------------------------------
    # The regression that produced "สว่างแสบตามาก". Scoped to this file: HIT_FX_DEFS and BOLT_TINT
    # elsewhere are over-range ON PURPOSE, because a sheet has no other way to reach the HDR
    # buffer.
    paints_block = const_block(src, "PAINTS")
    for m in re.finditer(r"Color\(([^)]*)\)", paints_block):
        try:
            nums = [float(x) for x in m.group(1).split(",")]
        except ValueError:
            continue
        if any(v > GLOW_THRESHOLD for v in nums[:3]):
            fail(f"PAINTS colour over the glow threshold: Color({m.group(1)})")

    # --- F. the shared trail shape is well formed ----------------------------------------------
    # `TRAIL_STOPS` / `TRAIL_ALPHAS` are zipped positionally in `recolour`, so a length mismatch
    # silently truncates the ramp — the trail loses its tail and nothing says why.
    stops = [float(x) for x in re.findall(r"-?[\d.]+", const_block(src, "TRAIL_STOPS"))]
    alphas = [float(x) for x in re.findall(r"-?[\d.]+", const_block(src, "TRAIL_ALPHAS"))]
    if not stops or stops != sorted(stops) or stops[0] < 0.0 or stops[-1] > 1.0:
        fail(f"TRAIL_STOPS not sorted within 0..1: {stops}")
    if len(stops) != len(alphas):
        fail(f"TRAIL_STOPS/TRAIL_ALPHAS length mismatch: {len(stops)} vs {len(alphas)}")
    if any(not 0.0 <= a <= 1.0 for a in alphas):
        fail(f"TRAIL_ALPHAS outside 0..1: {alphas}")

    # --- G. every paint is one of the three legal shapes ---------------------------------------
    # Painted (four colour keys + `trail`), shorthand (`tint`), or routing-only (no colours at
    # all). `recolour` tests for `Fire_Color` to decide, so a half-written entry — colours but no
    # trail, or a trail but no colours — takes the painted branch and hits a missing key at
    # runtime, in the renderer, mid-fight.
    NAMED = ("Fire_Color", "Ball_Color", "Line_Color", "light")
    for pid, body in paint_entries(paints_block).items():
        has = [k for k in NAMED if f'"{k}"' in body]
        n_trail = trail_len(body)
        n_tint = tint_len(body)
        if n_tint:
            if n_tint != 2:
                fail(f'PAINTS["{pid}"]: "tint" takes exactly [edge, core], found {n_tint}')
            continue
        if not has and n_trail == 0:
            continue          # routing-only, e.g. `blast`
        if len(has) != len(NAMED):
            fail(f'PAINTS["{pid}"]: painted but missing {sorted(set(NAMED) - set(has))}')
        if n_trail != len(stops) - 1:
            fail(f'PAINTS["{pid}"]: "trail" takes {len(stops) - 1} colours '
                 f'(the last stop wraps to the first), found {n_trail}')

    # --- H. the fan cannot roll a leg under the floor -----------------------------------------

    # `fan_legs()` derives the bound, so this only catches a spread that leaves no legs at all.
    for m in re.finditer(r'"fan"\s*:\s*([\d.]+)', src):
        if float(m.group(1)) > 90.0:
            fail(f'"fan": {m.group(1)} — over 90 degrees, every outer leg rolls under the floor')

    for p in problems:
        print(f"VFX TABLE ERROR: {p}", file=sys.stderr)
    if problems:
        print(f"\n{len(problems)} problem(s) in the VFX tables.", file=sys.stderr)
        return 1
    print(f"vfx tables ok — {len(paths)} scene paths, {len(paints)} paints, "
          f"{len(sim_skills)} sim skills cross-checked")
    return 0


if __name__ == "__main__":
    sys.exit(main())
